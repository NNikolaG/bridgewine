/* Split one #UD round trip: ud2 -> VEH entry, and VEH return -> resume (rdtsc). */
#include <windows.h>
#include <stdio.h>
#include <x86intrin.h>

static volatile unsigned long long t_veh;
static LONG CALLBACK handler(EXCEPTION_POINTERS *ep)
{
    t_veh = __rdtsc();
    ep->ContextRecord->Rip += 2;
    return EXCEPTION_CONTINUE_EXECUTION;
}

int main(int argc, char **argv)
{
    int n = argc > 1 ? atoi(argv[1]) : 100000;
    unsigned long long in = 0, out = 0, t0, t1, c0, c1;
    LARGE_INTEGER f, a, b;
    AddVectoredExceptionHandler(1, handler);
    QueryPerformanceFrequency(&f);
    QueryPerformanceCounter(&a); c0 = __rdtsc();
    for (int i = 0; i < n; i++)
    {
        t0 = __rdtsc();
        __asm__ volatile("ud2");
        t1 = __rdtsc();
        in += t_veh - t0; out += t1 - t_veh;
    }
    c1 = __rdtsc(); QueryPerformanceCounter(&b);
    double us_per_tick = (double)(b.QuadPart - a.QuadPart) * 1e6 / f.QuadPart / (double)(c1 - c0);
    printf("ud2 -> VEH: %.2f us, VEH -> resume: %.2f us\n", in * us_per_tick / n, out * us_per_tick / n);
    return 0;
}
