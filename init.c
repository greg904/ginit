#define _GNU_SOURCE

#include <stdbool.h>
#include <stdio.h>
#include <string.h>

#include <fcntl.h>
#include <grp.h>
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

#include "init-config.h"
#include "rtnl.h"

static void mount_bubble()
{
    if (mount("/dev/nvme0n1p2", "/bubble", "btrfs", 0, "subvol=/@bubble") == -1)
        perror("mount(/bubble)");
}

static void mount_special_fs()
{
    const unsigned long tmpfs_flags = MS_NOATIME | MS_NODEV | MS_NOEXEC | MS_NOSUID;

    if (mount("none", "/tmp", "tmpfs", tmpfs_flags, NULL) == -1)
        perror("mount(/tmp)");
    if (mount("none", "/run", "tmpfs", tmpfs_flags, NULL) == -1)
        perror("mount(/run)");
    if (mount("none", "/proc", "proc", 0, NULL) == -1)
        perror("mount(/proc)");
    if (mount("none", "/sys", "sysfs", 0, NULL) == -1)
        perror("mount(/sys)");
    if (mkdir("/dev/shm", 1744) == -1) {
        perror("mkdir(/dev/shm)");
    } else if (mount("none", "/dev/shm", "tmpfs", tmpfs_flags, NULL) == -1) {
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

static void setup_network()
{
    struct rtnl_addr_msg eth0_addr_msg;
    const unsigned char local_addr[4] = { 192, 168, 1, 26 };
    const unsigned char broadcast_addr[4] = { 255, 255, 255, 0 };
    rtnl_addr_msg_new(&eth0_addr_msg, local_addr, local_addr, broadcast_addr);

    struct rtnl_link_msg eth0_link_msg;
    rtnl_link_msg_set(&eth0_link_msg, 2, IFF_UP, IFF_UP);

    struct rtnl_link_msg lo_link_msg;
    rtnl_link_msg_set(&lo_link_msg, 1, IFF_UP, IFF_UP);

    struct rtnl_route_msg eth0_route_msg;
    const unsigned char gateway_addr[4] = { 192, 168, 1, 254 };
    rtnl_route_msg_new(&eth0_route_msg, 2, gateway_addr);

    struct rtnl *r = rtnl_open();
    if (r == NULL)
        return;

    if (rtnl_send(r, &eth0_addr_msg.hdr)) {
        struct nlmsghdr *hdr;
        ssize_t len = rtnl_recv(r, &hdr);
        if (len != -1) {
            int error = rtnl_get_error(hdr, len);
            if (error != 0)
                fprintf(stderr, "RTM_NEWADDR: %d\n", error);
        }
    }

    if (rtnl_send(r, &lo_link_msg.hdr)) {
        struct nlmsghdr *hdr;
        ssize_t len = rtnl_recv(r, &hdr);
        if (len != -1) {
            int error = rtnl_get_error(hdr, len);
            if (error != 0)
                fprintf(stderr, "RTM_SETLINK: %d\n", error);
        }
    }

    if (rtnl_send(r, &eth0_link_msg.hdr)) {
        struct nlmsghdr *hdr;
        ssize_t len = rtnl_recv(r, &hdr);
        if (len != -1) {
            int error = rtnl_get_error(hdr, len);
            if (error != 0)
                fprintf(stderr, "RTM_SETLINK: %d\n", error);
        }
    }

    if (rtnl_send(r, &eth0_route_msg.hdr)) {
        struct nlmsghdr *hdr;
        ssize_t len = rtnl_recv(r, &hdr);
        if (len != -1) {
            int error = rtnl_get_error(hdr, len);
            if (error != 0)
                fprintf(stderr, "RTM_NEWROUTE: %d\n", error);
        }
    }

    rtnl_close(r);
}

static void run_udevadm(char *const argv[]) {
    pid_t pid;
    char *const envp[] = { CONFIG_PATH, NULL };
    if (posix_spawn(&pid, CONFIG_UDEVADM, NULL, NULL, argv, envp) != 0) {
        perror("posix_spawn(" CONFIG_UDEVADM ")");
        return;
    }

    int code;
    if (waitpid(pid, &code, 0) == -1) {
        perror("waitpid(" CONFIG_UDEVADM ")");
        return;
    }

    if (code != 0)
        fputs(CONFIG_UDEVADM " exited with non-zero code", stderr);
}

/**
 * Starts `udev` and initialize devices.
 */
static void start_udev()
{
    char *const envp[] = { CONFIG_PATH, NULL };
    char *const deamon_argv[] = { CONFIG_UDEVD, NULL };
    pid_t daemon_pid;
    if (posix_spawn(&daemon_pid, CONFIG_UDEVD, NULL, NULL, deamon_argv, envp) != 0) {
        perror("posix_spawn(" CONFIG_UDEVD ")");
        return;
    }

#define RUN_UDEVADM(...) do { \
        char *const argv[] = { CONFIG_UDEVADM, __VA_ARGS__, NULL }; \
        run_udevadm(argv); \
    } while (0)
    RUN_UDEVADM("trigger", "--type", "subsystems", "--action=add");
    RUN_UDEVADM("trigger", "--type", "devices", "--action=add");
#undef RUN_UDEVADM
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
            if (dup2(tty, STDIN_FILENO) == -1 ||
                    dup2(tty, STDOUT_FILENO) == -1 ||
                    dup2(tty, STDERR_FILENO) == -1) {
                perror("dup2(/dev/tty0)");
            } else if (ioctl(tty, TIOCSCTTY, 1) == -1) {
                perror("ioctl(TIOCSCTTY)");
            }
            if (close(tty) == -1)
                perror("close()");
        }

        if (setgroups(sizeof(config_user_groups) / sizeof(config_user_groups[0]), config_user_groups) == -1) {
            perror("setgroups()");
            _exit(1);
        }
        if (setresgid(config_user_gid, config_user_gid, config_user_gid) == -1) {
            perror("setresgid()");
            _exit(2);
        }
        if (setresuid(config_user_uid, config_user_uid, config_user_uid) == -1) {
            perror("setresuid()");
            _exit(3);
        }

        if (chdir(CONFIG_USER_HOME) == -1)
            perror("chdir()");

        char *const argv[] = { "/usr/bin/sway", NULL };
        char *const envp[] = {
            "HOME=" CONFIG_USER_HOME,
            "MOZ_ENABLE_WAYLAND=1",
            CONFIG_PATH,
            "WLR_SESSION=direct",
            "XDG_RUNTIME_DIR=/home/greg/xdg-runtime-dir",
            "XDG_SEAT=seat0",
            NULL,
        };
        execvpe("/usr/bin/sway", argv, envp);
        perror("execvpe(/usr/bin/sway)");
        _exit(4);
    }
    return child;
}

/**
 * Pipes `stdout` and `stderr` to `/dev/kmsg` (this file contains the messages
 * that are shown by `dmesg`). This requires `/dev` to be already mounted.
 */
static void pipe_stdout_to_kmsg() {
    int kmsg_fd = open("/dev/kmsg", O_WRONLY | O_CLOEXEC);
    if (kmsg_fd == -1) {
        perror("open(/dev/kmsg)");
    } else {
        if (dup2(kmsg_fd, STDOUT_FILENO) == -1 || dup2(kmsg_fd, STDERR_FILENO) == -1)
            perror("dup2(/dev/kmsg)");
        if (close(kmsg_fd) == -1)
            perror("close(/dev/kmsg)");
    }
}

int main()
{
    if (close(STDIN_FILENO) == -1)
        perror("close(/dev/stdin)");

    if (mount("none", "/dev", "devtmpfs", 0, NULL) == -1) {
        perror("mount(/dev)");
    } else {
        pipe_stdout_to_kmsg();
    }

    mount_special_fs();
    mount_bubble();
    set_backlight_brightness();
    limit_battery_charge();
    set_sysctl_opts();
    setup_network();

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
