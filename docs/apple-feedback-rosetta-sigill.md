# Feedback Assistant draft: Rosetta 2 SIGILL delivery for illegal instructions is very slow

**Title:** Rosetta 2: delivering SIGILL for an illegal x86_64 instruction takes ~27 µs per round trip, making Windows games with anti-tamper unplayable (Wine, CrossOver)

**Area:** Rosetta

**Type:** Performance

## Description

Under Rosetta 2, each x86_64 `ud2` instruction that raises SIGILL and returns from a
handler takes about 26.7 µs. A plain macOS x86_64 C program with a SIGILL handler that
does `uc_mcontext->__ss.__rip += 2` and returns, executing `ud2` 200,000 times in a loop,
measures 26.73 µs per ud2/SIGILL round trip.

The same pattern under Wine (WineHQ 11.21), a Windows x86_64 program using
`AddVectoredExceptionHandler` to skip `ud2`, measures 33.2 µs per exception. Rosetta and
the kernel account for about 80% of that.

Profiling shows the time is mostly system time. Two threads are busy: the faulting thread
and the `com.apple.rosetta.exceptionserver` thread (Mach exception, then the Rosetta
exception server, then signal delivery).

Some Windows games' anti-tamper deliberately executes illegal instructions at a high rate
and handles them in a vectored exception handler. Under Rosetta this per-exception cost
makes those games unplayable.

The game below does not use `ud2`. It executes single-byte opcodes that are invalid in
64-bit mode: 0x60 (PUSHA), 0x16 (PUSH SS), 0x27 (DAA), 0xD5 (AAD), 0x3F (AAS), 0x1F, 0x17,
0x07, 0x06. The attached `sigill.c` uses `ud2` by default; run it with a second argument
`pusha` to execute 0x60 instead. We only measured the `ud2` case.

## Steps to reproduce

1. Save the attached `sigill.c`.
2. Build and run it under Rosetta 2:
   `clang -arch x86_64 -O2 -o sigill sigill.c && arch -x86_64 ./sigill 200000`
   (or `arch -x86_64 ./sigill 200000 pusha` for opcode 0x60)
3. Note the reported time per SIGILL.
4. Optional: build the attached `vehbench.c` as a Windows x86_64 executable and run it under
   Wine to see the same cost through Wine's vectored exception path.

## Expected vs actual results

- **Expected:** exception delivery cost roughly comparable to native exception delivery,
  which we estimate at around 1-2 µs. (This is an expectation, not a measurement made on
  this machine.)
- **Actual:** 26.73 µs per ud2/SIGILL round trip (plain macOS program under Rosetta 2);
  33.2 µs per exception under Wine 11.21.

## Impact

Age of Empires III: Definitive Edition (Steam) raises 14,000-19,000
`EXCEPTION_ILLEGAL_INSTRUCTION` per second continuously. Under Rosetta that alone costs
about half a CPU core or more and stalls the game: frozen screens, and audio looping
because the game cannot refill its buffers.

The same freeze reproduces in three independent Wine builds on this Mac: WineHQ 11.21,
our own patched Wine, and CrossOver 26.3 (CodeWeavers, Wine 11.0 based). Wine-side changes
(CrossOver's Rosetta fixes, Mach-semaphore sync) did not change the exception rate or the
freeze, so the cost is in Rosetta's exception delivery, not in Wine.

Related public reports (none of them gives an Apple Feedback or radar number; we found no
existing report for this issue as of 2026-11-03):

- Age of Empires II DE got the same protection in its June 2, 2026 update. Since then it
  loads for minutes and freezes for 1-3 s in matches under CrossOver 26.2 on M-series Macs:
  https://forums.ageofempires.com/t/bug-performance-issue-wrt-2026-june-02-patch/286862
- Same AoE II DE update under Sikarugir/Wine (crash with 0xC000001D, illegal instruction):
  https://github.com/Sikarugir-App/Sikarugir/issues/233
- Age of Empires IV: profiling showed substantial CPU time in Rosetta's exception handling:
  https://github.com/tarikbc/macgames/pull/5

## Configuration

- MacBook Pro, Apple M5
- macOS 27.0
- Rosetta 2
- WineHQ 11.21 (for the Wine measurement); freeze also reproduced in CrossOver 26.3
- Measured 2026-11-01; CrossOver 26.3 check 2026-11-03

## Attachments

### sigill.c (macOS x86_64, run under Rosetta 2)

```c
/* Cost of SIGILL delivery for ud2 under Rosetta 2, without Wine. */
#include <signal.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/ucontext.h>
#include <time.h>

/* argv[2] == "pusha": execute 0x60 (PUSHA, invalid in 64-bit mode, 1 byte) instead. */
static int skip = 2;

static void handler(int sig, siginfo_t *si, void *ctx)
{
    ((ucontext_t *)ctx)->uc_mcontext->__ss.__rip += skip;
}

int main(int argc, char **argv)
{
    int n = argc > 1 ? atoi(argv[1]) : 100000;
    int pusha = argc > 2 && !strcmp(argv[2], "pusha");
    struct sigaction sa = { .sa_sigaction = handler, .sa_flags = SA_SIGINFO };
    struct timespec a, b;
    if (pusha) skip = 1;
    sigaction(SIGILL, &sa, NULL);
    clock_gettime(CLOCK_MONOTONIC, &a);
    if (pusha)
        for (int i = 0; i < n; i++) __asm__ volatile(".byte 0x60");
    else
        for (int i = 0; i < n; i++) __asm__ volatile("ud2");
    clock_gettime(CLOCK_MONOTONIC, &b);
    double us = ((b.tv_sec - a.tv_sec) * 1e9 + (b.tv_nsec - a.tv_nsec)) / 1e3 / n;
    printf("%d SIGILL, %.2f us each\n", n, us);
    return 0;
}
```

### vehbench.c (Windows x86_64, run under Wine)

```c
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
```
