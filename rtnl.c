#include <stdbool.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include <sys/socket.h>
#include <unistd.h>

#include "rtnl.h"

struct rtnl *rtnl_open() {
    struct rtnl *r = malloc(sizeof(struct rtnl));
    if (r == NULL) {
        fputs("failed to allocate memory in rtnl_open\n", stderr);
        return NULL;
    }

    int fd = socket(AF_NETLINK, SOCK_RAW | SOCK_CLOEXEC, NETLINK_ROUTE);
    if (fd == -1) {
        perror("socket(NETLINK_ROUTE)");
        free(r);
        return NULL;
    }

    r->fd = fd;
    r->seq = 0;

    return r;
}

ssize_t rtnl_recv(struct rtnl *r, struct nlmsghdr **hdr) {
    struct sockaddr_nl nl_addr = {};
    nl_addr.nl_family = AF_NETLINK;

    socklen_t nl_addr_len = sizeof(nl_addr);
    ssize_t len = recvfrom(r->fd, NULL, 0, MSG_PEEK | MSG_TRUNC, (struct sockaddr *)&nl_addr, &nl_addr_len);
    if (len == -1) {
        perror("recvfrom(NETLINK_ROUTE)");
        return -1;
    }

    void *buf = malloc(len);
    if (buf == NULL) {
        fputs("failed to allocate memory in rtnl_recv\n", stderr);
        return -1;
    }

    nl_addr_len = sizeof(nl_addr);
    ssize_t ret = recvfrom(r->fd, buf, len, 0, (struct sockaddr *)&nl_addr, &nl_addr_len);
    if (ret == -1) {
        perror("recvfrom(NETLINK_ROUTE)");
        free(buf);
        return -1;
    }

    *hdr = (struct nlmsghdr*)buf;
    return len;
}

int rtnl_get_error(struct nlmsghdr *hdr, ssize_t len) {
    for (; NLMSG_OK(hdr, len); hdr = NLMSG_NEXT(hdr, len)) {
        if (hdr->nlmsg_type == NLMSG_ERROR) {
            struct nlmsgerr *err = (struct nlmsgerr *)NLMSG_DATA(hdr);
            return err->error;
        }
    }
    return 0;
}

bool rtnl_send(struct rtnl *r, struct nlmsghdr *hdr) {
    hdr->nlmsg_seq = ++r->seq;

    struct sockaddr_nl nl_addr = {};
    nl_addr.nl_family = AF_NETLINK;
    struct iovec iov = { (void*)hdr, hdr->nlmsg_len };
    struct msghdr msg = { (void*)&nl_addr, sizeof(nl_addr), &iov, 1, NULL, 0, 0 };
    int ret = sendmsg(r->fd, &msg, 0) != -1;
    if (ret == -1) {
        perror("sendmsg(NETLINK_ROUTE)");
        return false;
    }

    return true;
}

void rtnl_addr_msg_new(struct rtnl_addr_msg *m, const unsigned char prefix_addr[4], const unsigned char dest_addr[4], const unsigned char broadcast_addr[4]) {
    m->hdr.nlmsg_len = sizeof(*m);
    m->hdr.nlmsg_type = RTM_NEWADDR;
    m->hdr.nlmsg_flags = NLM_F_REQUEST | NLM_F_CREATE | NLM_F_EXCL | NLM_F_ACK;
    m->hdr.nlmsg_pid = 0;

    m->ifa.ifa_family = AF_INET;
    m->ifa.ifa_prefixlen = 24;
    m->ifa.ifa_flags = 0;
    m->ifa.ifa_scope = RT_SCOPE_UNIVERSE;
    m->ifa.ifa_index = 2;

    m->prefix_addr_attr.rta_type = IFA_LOCAL;
    m->prefix_addr_attr.rta_len = RTA_LENGTH(sizeof(m->prefix_addr));
    memcpy(m->prefix_addr, prefix_addr, sizeof(m->prefix_addr));

    m->dest_addr_attr.rta_type = IFA_ADDRESS;
    m->dest_addr_attr.rta_len = RTA_LENGTH(sizeof(m->dest_addr));
    memcpy(m->dest_addr, dest_addr, sizeof(m->dest_addr));

    m->broadcast_addr_attr.rta_type = IFA_BROADCAST;
    m->broadcast_addr_attr.rta_len = RTA_LENGTH(sizeof(m->broadcast_addr));
    memcpy(m->broadcast_addr, broadcast_addr, sizeof(m->broadcast_addr));
}

void rtnl_link_msg_set(struct rtnl_link_msg *m, int interface_index, unsigned int flags, unsigned int flags_mask) {
    m->hdr.nlmsg_len = sizeof(*m);
    m->hdr.nlmsg_type = RTM_SETLINK;
    m->hdr.nlmsg_flags = NLM_F_REQUEST | NLM_F_ACK;
    m->hdr.nlmsg_pid = 0;

    m->ifi.ifi_family = AF_UNSPEC;
    m->ifi.ifi_type = 0;
    m->ifi.ifi_index = interface_index;
    m->ifi.ifi_flags = flags;
    m->ifi.ifi_change = flags_mask;
}

void rtnl_route_msg_new(struct rtnl_route_msg *m, int interface_index, const unsigned char gateway_addr[4]) {
    m->hdr.nlmsg_len = sizeof(*m);
    m->hdr.nlmsg_type = RTM_NEWROUTE;
    m->hdr.nlmsg_flags = NLM_F_REQUEST | NLM_F_CREATE | NLM_F_EXCL | NLM_F_ACK;
    m->hdr.nlmsg_pid = 0;

    m->rt.rtm_family = AF_INET;
    m->rt.rtm_dst_len = 0;
    m->rt.rtm_src_len = 0;
    m->rt.rtm_tos = 0;
    m->rt.rtm_table = RT_TABLE_MAIN;
    m->rt.rtm_protocol = RTPROT_BOOT;
    m->rt.rtm_scope = RT_SCOPE_UNIVERSE;
    m->rt.rtm_type = RTN_UNICAST;
    m->rt.rtm_flags = 0;

    m->gateway_addr_attr.rta_type = RTA_GATEWAY;
    m->gateway_addr_attr.rta_len = RTA_LENGTH(sizeof(m->gateway_addr));
    memcpy(m->gateway_addr, gateway_addr, sizeof(m->gateway_addr));

    m->interface_index_attr.rta_type = RTA_OIF;
    m->interface_index_attr.rta_len = RTA_LENGTH(sizeof(m->interface_index));
    m->interface_index = interface_index;
}

bool rtnl_close(struct rtnl *r) {
    bool ok = true;
    if (r->fd != -1 && close(r->fd) == -1) {
        perror("close(NETLINK_ROUTE)");
        ok = false;
    }

    free(r);
    return ok;
}
