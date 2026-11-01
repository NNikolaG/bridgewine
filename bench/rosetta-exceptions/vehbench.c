/* Cost of a #UD exception handled by a vectored handler (EXCEPTION_CONTINUE_EXECUTION). */
#include <windows.h>
#include <stdio.h>

static LONG CALLBACK handler(EXCEPTION_POINTERS *ep)
{
    if (ep->ExceptionRecord->ExceptionCode != EXCEPTION_ILLEGAL_INSTRUCTION)
        return EXCEPTION_CONTINUE_SEARCH;
    ep->ContextRecord->Rip += 2; /* skip ud2 */
    return EXCEPTION_CONTINUE_EXECUTION;
}

int main(int argc, char **argv)
{
    int n = argc > 1 ? atoi(argv[1]) : 100000;
    LARGE_INTEGER f, a, b;
    AddVectoredExceptionHandler(1, handler);
    QueryPerformanceFrequency(&f);
    QueryPerformanceCounter(&a);
    for (int i = 0; i < n; i++) __asm__ volatile("ud2");
    QueryPerformanceCounter(&b);
    double us = (double)(b.QuadPart - a.QuadPart) * 1e6 / f.QuadPart / n;
    printf("%d exceptions, %.2f us each, %.0f per second per thread\n", n, us, 1e6 / us);
    return 0;
}
