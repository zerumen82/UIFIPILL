#ifndef _BYTESWAP_H
#define _BYTESWAP_H
#include <stdint.h>
#define bswap_16(x) ((uint16_t)((((x) >> 8) & 0xff) | (((x) & 0xff) << 8)))
#define bswap_32(x) ((uint32_t)((((x) >> 24) & 0xff) | (((x) >> 8) & 0xff00) | (((x) & 0xff00) << 8) | (((x) & 0xff) << 24)))
#endif
