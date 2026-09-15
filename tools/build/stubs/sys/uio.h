#ifndef _SYS_UIO_H
#define _SYS_UIO_H
#include <stdint.h>
#include <winsock2.h>
#define IOV_MAX 1024
struct iovec {
    void  *iov_base;
    size_t iov_len;
};
#endif
