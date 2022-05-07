#define _GNU_SOURCE

#include <stdbool.h>
#include <stdio.h>
#include <string.h>

#include <fcntl.h>
#include <libnetlink.h>
#include <linux/netlink.h>
#include <linux/rtnetlink.h>
#include <net/if.h>
#include <spawn.h>
#include <sys/ioctl.h>
#include <sys/mount.h>
#include <sys/reboot.h>
#include <sys/socket.h>
#include <sys/stat.h>
#include <sys/wait.h>
#include <termios.h>
#include <unistd.h>

#define TMPFS_FLAGS MS_NOATIME | MS_NODEV | MS_NOEXEC | MS_NOSUID

static void mount_bubble()
{
	if (mount("/dev/nvme0n1p3", "/bubble", "ext4", 0, NULL) == -1)
		perror("mount(/bubble)");
}

static void mount_special_fs()
{
	if (mount("none", "/tmp", "tmpfs", TMPFS_FLAGS, NULL) == -1)
		perror("mount(/tmp)");
	if (mount("none", "/proc", "proc", 0, NULL) == -1)
		perror("mount(/proc)");
	if (mount("none", "/sys", "sysfs", 0, NULL) == -1)
		perror("mount(/sys)");
	if (mount("none", "/dev", "devtmpfs", 0, NULL) == -1) {
		perror("mount(/dev)");
		return;
	}
	if (mkdir("/dev/shm", 1744) == -1) {
		perror("mkdir(/dev/shm)");
	} else if (mount("none", "/dev/shm", "tmpfs", TMPFS_FLAGS, NULL) == -1) {
		perror("mount(/dev/shm)");
	}
	if (mkdir("/dev/pts", 744) == -1) {
		perror("mkdir(/dev/pts)");
	} else if (mount("none", "/dev/pts", "devpts", 0, NULL) == -1) {
		perror("mount(/dev/pts)");
	}
}

static void open_write_close(const char *file, const char *str)
{
	int fd = open(file, O_WRONLY);
	if (fd == -1) {
		char buf[256];
		snprintf(buf, sizeof(buf) / sizeof(buf[0]), "open(%s)", file);
		perror(buf);
	}
	if (write(fd, str, strlen(str)) == -1) {
		char buf[256];
		snprintf(buf, sizeof(buf) / sizeof(buf[0]), "write(%s)", file);
		perror(buf);
	}
	if (close(fd) == -1) {
		char buf[256];
		snprintf(buf, sizeof(buf) / sizeof(buf[0]), "close(%s)", file);
		perror(buf);
	}
}

static void set_backlight_brightness()
{
	open_write_close("/sys/class/backlight/nv_backlight/brightness", "80");
}

static void limit_battery_charge()
{
	open_write_close("/sys/class/power_supply/BAT0/charge_control_end_threshold", "80");
}

static void set_sysctl_opts() {
	open_write_close("/proc/sys/fs/protected_symlinks", "1");
	open_write_close("/proc/sys/fs/protected_hardlinks", "1");
	open_write_close("/proc/sys/fs/protected_fifos", "1");
	open_write_close("/proc/sys/fs/protected_regular", "1");
}

static void bring_if_up(const char *name)
{
	int fd = socket(PF_INET, SOCK_DGRAM, 0);
	if (fd == -1) {
		perror("socket()");
		return;
	}

	struct ifreq ifr = {};
	strcpy(ifr.ifr_name, name);

	if (ioctl(fd, SIOCGIFFLAGS, &ifr) == -1) {
		perror("ioctl(SIOCGIFFLAGS)");
		if (close(fd) == -1)
			perror("close()");
		return;
	}

	ifr.ifr_flags |= IFF_UP;

	if (ioctl(fd, SIOCSIFFLAGS, &ifr) == -1)
		perror("ioctl(SIOCSIFFLAGS)");

	if (close(fd) == -1)
		perror("close");
}

static void set_eth0_addr()
{
    struct {
        struct nlmsghdr hdr;
        struct ifaddrmsg ifa;
        char buf[256];
    } msg = {};

    msg.hdr.nlmsg_len = NLMSG_LENGTH(sizeof(msg.ifa));
    msg.hdr.nlmsg_flags = NLM_F_REQUEST | NLM_F_CREATE | NLM_F_EXCL;
    msg.hdr.nlmsg_type = RTM_NEWADDR;

    msg.ifa.ifa_family = AF_INET;
    msg.ifa.ifa_prefixlen = 24;

    unsigned char addr[4] = { 192, 168, 1, 26 };
    if (addattr_l(&msg.hdr, sizeof(msg), IFA_LOCAL, &addr, sizeof(addr)) == -1) {
        perror("addattr_l()");
        return;
    }
    if (addattr_l(&msg.hdr, sizeof(msg), IFA_ADDRESS, &addr, sizeof(addr)) == -1) {
        perror("addattr_l()");
        return;
    }

    unsigned char brd_addr[4] = { 255, 255, 255, 0 };
    if (addattr_l(&msg.hdr, sizeof(msg), IFA_BROADCAST, &brd_addr, sizeof(brd_addr)) == -1) {
        perror("addattr_l()");
        return;
    }
    
    struct rtnl_handle rth;
    if (rtnl_open(&rth, 0) == -1) {
        perror("rtnl_open()");
        return;
    }

    if (rtnl_talk(&rth, &msg.hdr, 0, 0, NULL, NULL, NULL) == -1)
        perror("rtnl_talk()");
}

static void start_udev()
{
	char *const envp[] = { "PATH=/bin:/sbin:/usr/bin:/usr/sbin", NULL };

	char *const deamon_argv[] = { "/sbin/udevd", NULL };
	pid_t daemon_pid;
	if (posix_spawn(&daemon_pid, "/sbin/udevd", NULL, NULL, deamon_argv, envp) != 0) {
		perror("posix_spawn(/sbin/udevd)");
		return;
	}

	char *const trigger_argv[] = { "/sbin/udevadm", "trigger", "--action=add", NULL };
	pid_t trigger_pid;
	if (posix_spawn(&trigger_pid, "/sbin/udevadm", NULL, NULL, trigger_argv, envp) != 0) {
		perror("posix_spawn(/sbin/udevadm)");
	} else {
		int code;
		if (waitpid(trigger_pid, &code, 0) == -1)
			perror("waitpid(/sbin/udevadm)");
	}

	char *const settle_argv[] = { "/sbin/udevadm", "settle", NULL };
	pid_t settle_pid;
	if (posix_spawn(&settle_pid, "/sbin/udevadm", NULL, NULL, settle_argv, envp) != 0) {
		perror("posix_spawn(/sbin/udevadm)");
	} else {
		int code;
		if (waitpid(settle_pid, &code, 0) == -1)
			perror("waitpid(/sbin/udevadm)");
	}
}

static pid_t start_sway()
{
	pid_t child = fork();
	if (child == -1) {
		perror("fork()");
		return -1;
	} else if (child == 0) {
		if (setsid() == -1)
			perror("setsid()");

		int tty = open("/dev/tty0", O_RDWR | O_NOCTTY);
		if (tty == -1) {
			perror("open(/dev/tty0)");
		} else {
			if (dup2(tty, 0) == -1 || dup2(tty, 1) == -1 || dup2(tty, 2) == -1) {
				perror("dup2(/dev/tty0)");
			} else if (ioctl(tty, TIOCSCTTY, 1) == -1) {
				perror("ioctl(TIOCSCTTY)");
			}
			if (close(tty) == -1)
				perror("close()");
		}

		gid_t groups[] = { 10, 18, 23, 27, 1000 };
		if (setgroups(sizeof(groups) / sizeof(groups[0]), groups) == -1) {
			perror("setgroups()");
			_exit(1);
		}
		if (setresgid(1000, 1000, 1000) == -1) {
			perror("setresgid()");
			_exit(2);
		}
		if (setresuid(1000, 1000, 1000) == -1) {
			perror("setresuid()");
			_exit(3);
		}

		char *const argv[] = { "/usr/bin/sway", NULL };
		char *const envp[] = { "PATH=/bin:/sbin:/usr/bin:/usr/sbin", "XDG_RUNTIME_DIR=/run/me", "HOME=/home/me", NULL };
		execvpe("/usr/bin/sway", argv, envp);
		perror("execvpe(/usr/bin/sway)");
		_exit(4);
	}
	return child;
}

int main()
{
	mount_special_fs();
	mount_bubble();
	set_backlight_brightness();
	limit_battery_charge();
	set_sysctl_opts();
	bring_if_up("eth0");
	set_eth0_addr();
	bring_if_up("lo");
	// ip route add default via 192.168.1.254 dev eth0

	start_udev();
	start_sway();

	for (;;) {
		/* Reap zombie processes. */
		pid_t p = wait(NULL);
		if (p == -1) {
			perror("wait()");
			break;
		}
	}

	sync();
	reboot(RB_POWER_OFF);

	/* We should never get here. */
	return 0;
}
