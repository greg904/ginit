/*
 * Simple text editor with insert mode and command mode.
 *
 * At the start, the file is mmap-ed so that we don't have to keep a buffer with
 * what's visible on screen. Edits are stored in an ordered linked list and
 * committed on save.
 */

#include <assert.h>
#include <stdbool.h>
#include <stdio.h>
#include <stdint.h>
#include <string.h>

#include <fcntl.h>
#include <sys/ioctl.h>
#include <sys/mman.h>
#include <sys/stat.h>
#include <termios.h>
#include <unistd.h>

static struct winsize ws;

/* The file is mmap-ed. */
static char *file = NULL;
static uint32_t filesz = 0;

/* Position of the cursor in the file. */
static uint32_t off = 0;
/* Position of the first character in the first line in the viewport in the
 * file. */
static uint32_t voff = 0;
/* X coordinate of the viewport's left hand corner. Tabs are counted as
 * multiple characters. */
static uint32_t vx = 0;

/* When changing back and forth between lines of different length, we want to
 * preserve the cursor's column. */
static uint64_t prefercol = 0;

struct edit {
	uint32_t off;
	uint32_t oldsz;
	uint32_t newsz;
	void *new;
	struct edit *next;
};

static struct edit* edits = NULL;

static void setprefercol()
{
	/* Find start of current line. */
	uint32_t tmp = off;
	while (tmp != 0 && file[tmp - 1] != '\n')
		--tmp;

	prefercol = 0;
	for (; tmp != off; ++tmp) {
		assert(tmp < filesz);
		assert(file[tmp] != '\n');
		switch (file[tmp]) {
		case '\t':
			prefercol += 8;
			prefercol -= prefercol % 8;
			break;
		default:
			++prefercol;
			break;
		}
	}
}

static uint8_t getindent()
{
	uint32_t boundary = off;
	while (boundary != 0 && file[boundary - 1] != '\n' && file[boundary - 1] != '\t')
		--boundary;
	uint32_t chars = off - boundary;
	return 8 - (chars % 8);
}

static bool left()
{
	if (off == 0 || file[off - 1] == '\n')
		return true;
	--off;
	setprefercol();
	if (file[off] == '\t') {
		uint8_t indent = getindent();
		char seq[32];
		assert(sprintf(seq, "\x1b[%uD", indent) >= 0);
		if (write(0, seq, strlen(seq)) == -1) {
			perror("write() to stdout");
			return false;
		}
	} else {
		const char *seq = "\x1b[D";
		if (write(0, seq, strlen(seq)) == -1) {
			perror("write() to stdout");
			return false;
		}
	}
	return true;
}

static bool right()
{
	assert(off <= filesz);
	if (off == filesz || file[off] == '\n')
		return true;
	if (file[off] == '\t') {
		uint8_t indent = getindent();
		char seq[32];
		assert(sprintf(seq, "\x1b[%uC", indent) >= 0);
		if (write(0, seq, strlen(seq)) == -1) {
			perror("write() to stdout");
			return false;
		}
	} else {
		const char *seq = "\x1b[C";
		if (write(0, seq, strlen(seq)) == -1) {
			perror("write() to stdout");
			return false;
		}
	}
	++off;
	setprefercol();
	return true;
}

static bool up()
{
	if (off == 0)
		return true;

	/* Find end of previous line. */
	for (;;) {
		if (off == 0) {
			const char *seq = "\x1b[G";
			if (write(0, seq, strlen(seq)) == -1) {
				perror("write() to stdout");
				return false;
			}
			return true;
		}
		--off;
		if (file[off] == '\n')
			break;
	}

	/* Find start of previous line. */
	while (off != 0 && file[off - 1] != '\n')
		--off;

	uint32_t col = 0;
	for (;;) {
		assert(col <= prefercol);
		assert(off < filesz);
		if (col == prefercol || file[off] == '\n')
			break;
		if (file[off] == '\t') {
			uint32_t newcol = col + 8;
			newcol -= newcol % 8;
			if (newcol > prefercol)
				break;
			col = newcol;
		} else {
			++col;
		}
		++off;
	}

	char seq[32];
	assert(col != UINT32_MAX);
	assert(sprintf(seq, "\x1b[A\x1b[%uG", col + 1) >= 0);
	if (write(0, seq, strlen(seq)) == -1) {
		perror("write() to stdout");
		return false;
	}

	return true;
}

static bool down()
{
	assert(off <= filesz);

	if (off == filesz)
		return true;

	/* Find start of next line. */
	uint32_t prev = off;
	for (;;) {
		++off;
		if (file[off - 1] == '\n')
			break;
		if (off == filesz) {
			char seq[32];
			assert(sprintf(seq, "\x1b[%uC", off - prev) >= 0);
			if (write(0, seq, strlen(seq)) == -1) {
				perror("write() to stdout");
				return false;
			}
			return true;
		}
	}

	uint32_t col = 0;
	for (;;) {
		assert(col <= prefercol);
		assert(off <= filesz);
		if (col == prefercol || off == filesz || file[off] == '\n')
			break;
		if (file[off] == '\t') {
			uint32_t newcol = col + 8;
			newcol -= newcol % 8;
			if (newcol > prefercol)
				break;
			col = newcol;
		} else {
			++col;
		}
		++off;
	}

	char seq[32];
	assert(col != UINT32_MAX);
	assert(sprintf(seq, "\n\x1b[%uG", col + 1) >= 0);
	if (write(0, seq, strlen(seq)) == -1) {
		perror("write() to stdout");
		return false;
	}

	return true;
}

bool termmode()
{
	struct termios t;
	if (tcgetattr(0, &t) == -1) {
		perror("tcgetattr()");
		return false;
	}
	t.c_lflag &= ~ICANON;
	t.c_lflag &= ~ECHO;
	if (tcsetattr(0, TCSANOW, &t) == -1) {
		perror("tcsetattr()");
		return false;
	}
	return true;
}

bool termorig()
{
	const char *seq = "\x1b[1;1H";
	if (write(0, seq, strlen(seq)) == -1) {
		perror("write() to stdout");
		return false;
	}
	return true;
}

bool termclear()
{
	const char *seq = "\x1b[2J";
	if (write(0, seq, strlen(seq)) == -1) {
		perror("write() to stdout");
		return false;
	}
	return true;
}

bool redraw()
{
	if (!termclear())
		return false;

	if (voff == filesz)
		return true;

	if (!termorig())
		return false;

	/* TODO: find a name for the variable */
	uint32_t tmp = voff;

	for (short y = 0; y != ws.ws_row; ++y) {
		if (y != 0) {
			if (write(0, "\n\r", 2) == -1) {
				perror("write() to stdout");
				return false;
			}
		}

		uint32_t skip = vx;
		uint64_t spaces = 0;
		while (skip != 0) {
			switch (file[tmp]) {
			case '\n':
				++tmp;
				if (tmp == filesz)
					return true;
				goto nextline;
			case '\t':
				if (skip < 8) {
					skip = 0;
					spaces = 8 - skip;
					break;
				}
				skip -= 8;
				break;
			default:
				--skip;
				break;
			}
			++tmp;
			if (tmp == filesz)
				return true;
		}

		uint64_t col = 0;
		while (col < ws.ws_col) {
			switch (file[tmp]) {
			case '\n':
				++tmp;
				if (tmp == filesz)
					return true;
				goto nextline;
			case '\t':
			{
				uint8_t indent = 8 - ((vx + col + spaces) & 7);
				assert(spaces <= UINT32_MAX - indent);
				spaces += indent;
				assert(col <= UINT32_MAX - indent);
				col += indent;
				break;
			}
			default:
				/* Don't move the cursor using an ANSI sequence
				 * because we don't want to convert numbers to
				 * string and back to numbers in the VT emulator
				 * again. */
				/* TODO: single write() call */
				while (spaces != 0) {
					if (write(0, " ", 1) == -1) {
						perror("write() to stdout");
						return false;
					}
					--spaces;
				}
				if (write(0, &file[tmp], 1) == -1) {
					perror("write() to stdout");
					return false;
				}
				++col;
			}
			++tmp;
			if (tmp == filesz)
				return true;
		}

nextline:
		continue;
	}

	return true;
}

int main(int argc, char *argv[])
{
	if (argc != 2) {
		fprintf(stderr, "Usage: %s [FILE]\n", argv[0]);
		return 1;
	}

	int fd = open(argv[1], O_RDWR | O_CREAT);
	if (fd == -1) {
		perror("open()");
		return 1;
	}

	struct stat s;
	if (fstat(fd, &s) == -1) {
		perror("fstat()");
		if (close(fd) == -1)
			perror("close()");
		return 1;
	}

	if (s.st_size > UINT32_MAX) {
		fprintf(stderr, "%s: cannot open '%s' because it is larger than 4GiB\n", argv[0], argv[1]);
		if (close(fd) == -1)
			perror("close()");
		return 1;
	}

	file = mmap(NULL, s.st_size, PROT_READ, MAP_SHARED, fd, 0);
	filesz = s.st_size;
	if (file == MAP_FAILED) {
		perror("mmap()");
		if (close(fd) == -1)
			perror("close()");
		return 1;
	}

	if (!termmode()) {
		if (munmap(file, s.st_size) == -1)
			perror("munmap()");
		if (close(fd) == -1)
			perror("close()");
		return 1;
	}

	if (ioctl(STDOUT_FILENO, TIOCGWINSZ, &ws) == -1) {
		perror("ioctl(..., TIOCGWINSZ)");
		if (munmap(file, s.st_size) == -1)
			perror("munmap()");
		if (close(fd) == -1)
			perror("close()");
		return 1;
	}

	redraw();
	termorig();

	for (;;) {
		char buf[4096];
		ssize_t bytes = read(1, buf, sizeof(buf));
		if (bytes == -1) {
			perror("read(1, ...)");
			break;
		} else if (bytes == 0) {
			break;
		}
		for (size_t j = 0; j < bytes; j++) {
			switch (buf[j]) {
			case 'q':
				return 0;
			case 'h':
				left();
				break;
			case 'j':
				down();
				break;
			case 'k':
				up();
				break;
			case 'l':
				right();
				break;
			}
		}
	}

	/* TODO: cleanup */

	return 0;
}
