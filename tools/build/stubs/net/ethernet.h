#ifndef _NET_ETHERNET_H
#define _NET_ETHERNET_H
#define ETHER_ADDR_LEN 6
struct ether_addr {
    unsigned char ether_addr_octet[6];
};
#endif
