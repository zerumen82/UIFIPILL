#ifndef _LINUX_WIRELESS_H
#define _LINUX_WIRELESS_H
#define SIOCSIWFREQ  0x8B04
#define SIOCGIWFREQ  0x8B05
#define SIOCSIWMODE  0x8B06
#define SIOCGIWMODE  0x8B07
#define IW_MODE_MONITOR 6
struct iwreq {
    char ifr_name[16];
    union { int mode; };
};
#endif
