#ifndef _SYS_UN_H
#define _SYS_UN_H
#define UNIX_PATH_MAX 108
struct sockaddr_un { short sun_family; char sun_path[108]; };
#endif
