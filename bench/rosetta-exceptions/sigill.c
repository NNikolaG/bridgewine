/* Cost of SIGILL delivery for ud2 under Rosetta 2, without Wine. */
#include <signal.h>
#include <stdio.h>
#include <stdlib.h>
#include <sys/ucontext.h>
#include <time.h>

static void handler(int sig, siginfo_t *si, void *ctx)
{
    ((ucontext_t *)ctx)->uc_mcontext->__ss.__rip += 2;
}

int main(int argc, char **argv)
{
    int n = argc > 1 ? atoi(argv[1]) : 100000;
    struct sigaction sa = { .sa_sigaction = handler, .sa_flags = SA_SIGINFO };
    struct timespec a, b;
    sigaction(SIGILL, &sa, NULL);
    clock_gettime(CLOCK_MONOTONIC, &a);
    for (int i = 0; i < n; i++) __asm__ volatile("ud2");
    clock_gettime(CLOCK_MONOTONIC, &b);
    double us = ((b.tv_sec - a.tv_sec) * 1e9 + (b.tv_nsec - a.tv_nsec)) / 1e3 / n;
    printf("%d SIGILL, %.2f us each\n", n, us);
    return 0;
}
