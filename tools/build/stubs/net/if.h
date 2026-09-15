#ifndef _NET_IF_H
#define _NET_IF_H
#define IF_NAMESIZE 16
#define IFNAMSIZ 16
struct ifreq { char ifr_name[16]; };
#endif
