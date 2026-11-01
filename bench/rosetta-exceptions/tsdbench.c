/* Cost of _thread_set_tsd_base (the GSBASE switch Wine makes twice per signal on macOS). */
#include <stdio.h>
#include <stdint.h>
#include <stdlib.h>
#include <time.h>
#include <pthread.h>
extern void _thread_set_tsd_base(uint64_t);
static uint64_t gs(int slot) { uint64_t v; __asm__ volatile("movq %%gs:(%1), %0" : "=r"(v) : "r"((uint64_t)slot * 8)); return v; }
int main(int argc, char **argv)
{
    int n = argc > 1 ? atoi(argv[1]) : 1000000;
    uint64_t *self = (uint64_t *)pthread_self(), base = 0;
    for (int i = 0; i < 512; i++)
        if (self[i] == (uint64_t)self && self[i + 1] == gs(1) && self[i + 2] == gs(2)) { base = (uint64_t)&self[i]; break; }
    if (!base) { puts("tsd base not found"); return 1; }
    struct timespec a, b;
    clock_gettime(CLOCK_MONOTONIC, &a);
    for (int i = 0; i < n; i++) _thread_set_tsd_base(base);
    clock_gettime(CLOCK_MONOTONIC, &b);
    printf("%d tsd_base, %.3f us each\n", n, ((b.tv_sec - a.tv_sec) * 1e9 + (b.tv_nsec - a.tv_nsec)) / 1e3 / n);
    return 0;
}
