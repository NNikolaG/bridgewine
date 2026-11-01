/* Cost of RtlCaptureContext + NtContinue (the return half of exception dispatch). */
#include <windows.h>
#include <stdio.h>

NTSTATUS WINAPI NtContinue(CONTEXT *, BOOLEAN);

int main(int argc, char **argv)
{
    int n = argc > 1 ? atoi(argv[1]) : 100000;
    LARGE_INTEGER f, a, b;
    static CONTEXT ctx;
    volatile int i = 0;
    QueryPerformanceFrequency(&f);
    QueryPerformanceCounter(&a);
    RtlCaptureContext(&ctx);
    if (++i < n) NtContinue(&ctx, FALSE);
    QueryPerformanceCounter(&b);
    double us = (double)(b.QuadPart - a.QuadPart) * 1e6 / f.QuadPart / n;
    printf("%d NtContinue, %.2f us each\n", n, us);
    return 0;
}
