#define _GNU_SOURCE

#include <stdbool.h>
#include <stdio.h>
#include <string.h>

#include <fcntl.h>
#include <grp.h>
// #include <libnetlink.h>
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
	if (mount("/dev/nvme0n1p2", "/bubble", "btrfs", 0, "subvol=/@bubble") == -1)
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
	int fd = open(file, O_WRONLY | O_CLOEXEC);
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
	int fd = socket(PF_INET, SOCK_DGRAM | SOCK_CLOEXEC, 0);
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

static bool nlmsg_add_attr(struct nlmsghdr *msg, int max_len, int type, const void *data, int data_len)
{
	int offset = NLMSG_ALIGN(msg->nlmsg_len);
	int rta_len = RTA_LENGTH(data_len);
	if (offset + rta_len > max_len)
		return false;

	struct rtattr *rta = (struct rtattr *)((void *)msg + offset);
	rta->rta_type = type;
	rta->rta_len = rta_len;

	memcpy(RTA_DATA(rta), data, data_len);
	msg->nlmsg_len = offset + rta_len;
	return true;
}

static void setup_eth0()
{
	// This is the message to set the IPv4 address.
	struct {
		struct nlmsghdr hdr;
		struct ifaddrmsg ifa;
		char buf[64];
	} addr_msg = {};
	addr_msg.hdr.nlmsg_len = NLMSG_LENGTH(sizeof(addr_msg.ifa));
	addr_msg.hdr.nlmsg_flags = NLM_F_REQUEST | NLM_F_CREATE | NLM_F_EXCL;
	addr_msg.hdr.nlmsg_seq = 0;
	addr_msg.hdr.nlmsg_type = RTM_NEWADDR;
	addr_msg.ifa.ifa_family = AF_INET;
	addr_msg.ifa.ifa_prefixlen = 24;
	const unsigned char addr[4] = { 192, 168, 1, 26 };
	const unsigned char brd_addr[4] = { 255, 255, 255, 0 };
	if (!nlmsg_add_attr(&addr_msg.hdr, sizeof(addr_msg), IFA_LOCAL, addr, sizeof(addr)) ||
			!nlmsg_add_attr(&addr_msg.hdr, sizeof(addr_msg), IFA_ADDRESS, addr, sizeof(addr)) ||
			!nlmsg_add_attr(&addr_msg.hdr, sizeof(addr_msg), IFA_BROADCAST, brd_addr, sizeof(brd_addr))) {
		fputs("nlmsg_add_attr(): buffer is too small\n", stderr);
		return;
	}

	// This is the message to set the default route.
	struct {
		struct nlmsghdr hdr;
		struct rtmsg rt;
		char buf[64];
	} rt_msg = {};
	rt_msg.hdr.nlmsg_len = NLMSG_LENGTH(sizeof(rt_msg.rt));
	rt_msg.hdr.nlmsg_flags = NLM_F_REQUEST | NLM_F_CREATE | NLM_F_EXCL;
	rt_msg.hdr.nlmsg_seq = 1;
	rt_msg.hdr.nlmsg_type = RTM_NEWROUTE;
	rt_msg.rt.rtm_family = AF_INET;
	rt_msg.rt.rtm_dst_len = 32;
	rt_msg.rt.rtm_table = RT_TABLE_MAIN;
	rt_msg.rt.rtm_protocol = RTPROT_BOOT;
	rt_msg.rt.rtm_scope = RT_SCOPE_LINK;
	rt_msg.rt.rtm_type = RTN_UNICAST;
	const unsigned char gw_addr[4] = { 192, 168, 1, 254 };
	if (!nlmsg_add_attr(&rt_msg.hdr, sizeof(rt_msg), RTA_GATEWAY, gw_addr, sizeof(gw_addr))) {
		fputs("nlmsg_add_attr(): buffer is too small\n", stderr);
		return;
	}
    
	int fd = socket(PF_NETLINK, SOCK_RAW | SOCK_CLOEXEC, NETLINK_ROUTE);
	if (fd == -1) {
		perror("socket(NETLINK_ROUTE)");
		return;
	}
	if (send(fd, &addr_msg, addr_msg.hdr.nlmsg_len, 0) == -1 ||
			send(fd, &rt_msg, rt_msg.hdr.nlmsg_len, 0) == -1)
		perror("close(NETLINK_ROUTE)");
	if (close(fd) == -1)
		perror("close(NETLINK_ROUTE)");
}

static void start_udev()
{
	char *const envp[] = { "PATH=/usr/bin", NULL };

	char *const deamon_argv[] = { "/lib/systemd/systemd-udevd", NULL };
	pid_t daemon_pid;
	if (posix_spawn(&daemon_pid, "/lib/systemd/systemd-udevd", NULL, NULL, deamon_argv, envp) != 0) {
		perror("posix_spawn(/lib/systemd/systemd-udevd)");
		return;
	}

	char *const trigger_argv[] = { "/usr/bin/udevadm", "trigger", "--action=add", NULL };
	pid_t trigger_pid;
	if (posix_spawn(&trigger_pid, "/usr/bin/udevadm", NULL, NULL, trigger_argv, envp) != 0) {
		perror("posix_spawn(/usr/bin/udevadm)");
	} else {
		int code;
		if (waitpid(trigger_pid, &code, 0) == -1)
			perror("waitpid(/usr/bin/udevadm)");
	}

	char *const settle_argv[] = { "/sbin/udevadm", "settle", NULL };
	pid_t settle_pid;
	if (posix_spawn(&settle_pid, "/usr/bin/udevadm", NULL, NULL, settle_argv, envp) != 0) {
		perror("posix_spawn(/usr/bin/udevadm)");
	} else {
		int code;
		if (waitpid(settle_pid, &code, 0) == -1)
			perror("waitpid(/usr/bin/udevadm)");
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

		int tty = open("/dev/tty0", O_RDWR | O_CLOEXEC | O_NOCTTY);
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

		gid_t groups[] = { 24, 25, 27, 29, 30, 44, 46, 109, 113, 119, 1000 };
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

		if (chdir("/home/greg") == -1)
			perror("chdir()");

		char *const argv[] = { "/usr/bin/sway", NULL };
		char *const envp[] = {
			"HOME=/home/greg",
			"PATH=/usr/bin",
			"WLR_SESSION=direct",
			"XDG_RUNTIME_DIR=/home/greg/xdg-runtime-dir",
			"XDG_SEAT=seat-main",
			NULL,
		};
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
	bring_if_up("lo");
	setup_eth0();
	bring_if_up("eth0");

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
