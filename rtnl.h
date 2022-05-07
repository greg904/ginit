#ifndef RTNL_H
#define RTNL_H

#include <stdint.h>

#include <unistd.h>
#include <linux/netlink.h>
#include <linux/rtnetlink.h>

struct rtnl {
    int fd;
    uint32_t seq;
};

struct rtnl_link_msg {
    struct nlmsghdr hdr;
    struct ifinfomsg ifi;
};

struct rtnl_addr_msg {
    struct nlmsghdr hdr;
    struct ifaddrmsg ifa;

    struct rtattr prefix_addr_attr;
    unsigned char prefix_addr[4];

    struct rtattr dest_addr_attr;
    unsigned char dest_addr[4];

    struct rtattr broadcast_addr_attr;
    unsigned char broadcast_addr[4];
};

struct rtnl_route_msg {
    struct nlmsghdr hdr;
    struct rtmsg rt;

    struct rtattr gateway_addr_attr;
    unsigned char gateway_addr[4];

    struct rtattr interface_index_attr;
    int interface_index;
};

struct rtnl *rtnl_open();
ssize_t rtnl_recv();
int rtnl_get_error(struct nlmsghdr *hdr, ssize_t len);
bool rtnl_send(struct rtnl *r, struct nlmsghdr *hdr);
void rtnl_addr_msg_new(struct rtnl_addr_msg *m, const unsigned char prefix_addr[4], const unsigned char dest_addr[4], const unsigned char broadcast_addr[4]);
void rtnl_link_msg_set(struct rtnl_link_msg *m, int interface_index, unsigned int flags, unsigned int flags_mask);
void rtnl_route_msg_new(struct rtnl_route_msg *m, int interface_index, const unsigned char gateway_addr[4]);
bool rtnl_close(struct rtnl *r);

#endif
