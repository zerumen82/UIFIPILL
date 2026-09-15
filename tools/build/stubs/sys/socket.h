#include <winsock2.h>
#include <windows.h>
#include <stdint.h>
#ifndef SOCK_STREAM
#define SOCK_STREAM 1
#endif
#ifndef SOCK_DGRAM
#define SOCK_DGRAM 2
#endif
#ifndef SOCK_RAW
#define SOCK_RAW 3
#endif
#ifndef AF_INET
#define AF_INET 2
#endif
#ifndef AF_PACKET
#define AF_PACKET 0
#endif
#ifndef PF_PACKET
#define PF_PACKET 0
#endif
