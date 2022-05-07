#ifndef INIT_CONFIG_H
#define INIT_CONFIG_H

#include <unistd.h>

#define CONFIG_PATH "PATH=/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin:/opt/bin"
#define CONFIG_UDEVD "/sbin/udevd"
#define CONFIG_UDEVADM "/sbin/udevadm"

#define CONFIG_USER_HOME "/home/greg"

static const uid_t config_user_uid = 1000;
static const gid_t config_user_gid = 1000;
static const gid_t config_user_groups[] = { 1000, 10, 18, 27, 97 };

#endif
