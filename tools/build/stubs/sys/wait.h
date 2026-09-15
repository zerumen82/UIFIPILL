#ifndef _SYS_WAIT_H
#define _SYS_WAIT_H
#define WNOHANG 1
#define WEXITSTATUS(s) (((s) & 0xff00) >> 8)
#define WIFEXITED(s)  (((s) & 0x7f) == 0)
#endif
