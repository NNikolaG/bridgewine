/* Floor: native arm64 macOS SIGILL round trip (udf), no Rosetta, no Wine.
 * Whatever arm64 Wine+FEX does on top of the kernel trap, it cannot beat this.
 *   clang -arch arm64 -O2 -o sigill_arm64 sigill_arm64.c && ./sigill_arm64 200000 */
#include <signal.h>
#include <stdio.h>
#include <stdlib.h>
#include <sys/ucontext.h>
#include <time.h>

static void handler(int sig, siginfo_t *si, void *ctx)
{
    ((ucontext_t *)ctx)->uc_mcontext->__ss.__pc += 4;
}

int main(int argc, char **argv)
{
    int n = argc > 1 ? atoi(argv[1]) : 100000;
    struct sigaction sa = { .sa_sigaction = handler, .sa_flags = SA_SIGINFO };
    struct timespec a, b;
    sigaction(SIGILL, &sa, NULL);
    clock_gettime(CLOCK_MONOTONIC, &a);
    for (int i = 0; i < n; i++) __asm__ volatile("udf #0");
    clock_gettime(CLOCK_MONOTONIC, &b);
    double us = ((b.tv_sec - a.tv_sec) * 1e9 + (b.tv_nsec - a.tv_nsec)) / 1e3 / n;
    printf("%d SIGILL (native arm64), %.2f us each\n", n, us);
    return 0;
}
