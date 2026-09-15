// win32_compat.h — POSIX-to-Windows compatibility header for MSYS2/MinGW64
// Force-include via: gcc -include win32_compat.h
#ifndef WIN32_COMPAT_H
#define WIN32_COMPAT_H

#define WIN32_LEAN_AND_MEAN
#define _WIN32_WINNT 0x0601  // Windows 7+

#include <winsock2.h>
#include <ws2tcpip.h>
#include <windows.h>

// Suppress Windows macro conflicts with reaver/bully enums
#undef NO_ERROR
#undef X509_CERT
#undef INTERFACE

// POSIX types not available in MinGW64
#ifndef timer_t
typedef int timer_t;
#endif

// POSIX -> Windows mappings
#define sleep(sec) Sleep((sec)*1000)
#define usleep(usec) Sleep((usec)/1000)

// Signal constants for Windows
#ifndef SIGALRM
#define SIGALRM 14
#endif

// Provide uint8_t if not defined
#ifndef UINT8_MAX
typedef unsigned char uint8_t;
#endif

#endif // WIN32_COMPAT_H
