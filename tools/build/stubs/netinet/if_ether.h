#include <stdint.h>
#define ETH_ALEN 6
struct ethhdr {
    unsigned char h_dest[6];
    unsigned char h_source[6];
    uint16_t h_proto;
};
