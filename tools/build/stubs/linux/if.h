#ifndef _LINUX_IF_H
#define _LINUX_IF_H
#define IFNAMSIZ 16
#define IFF_UP 0x1
#define IFF_PROMISC 0x100
#define IFF_BROADCAST 0x2
struct ifreq {
    char ifr_name[16];
    union { int ifr_flags; int ifr_ifindex; };
};
#endif
