#ifndef _LINUX_IF_ETHER_H
#define _LINUX_IF_ETHER_H
#include <stdint.h>
#define ETH_ALEN 6
#define ETH_P_ALL 0x0003
struct ethhdr {
    unsigned char h_dest[6];
    unsigned char h_source[6];
    uint16_t h_proto;
};
#endif
