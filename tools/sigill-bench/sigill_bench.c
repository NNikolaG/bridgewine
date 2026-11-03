/*
 * sigill_bench: mjeri cenu jednog EXCEPTION_ILLEGAL_INSTRUCTION koji VEH
 * odmah nastavi (EXCEPTION_CONTINUE_EXECUTION). Isti obrazac kao zastita u
 * AoE3 DE: opkodi koji ne postoje u 64-bitnom modu (PUSHA 0x60, DAA 0x27,
 * AAD 0xd5 ...). (ud2 namerno izostavljen: AoE3 ga ne koristi, a pod
 * WineHQ 11.21 + Rosetta ruzi proces van VEH-a.)
 *
 * Build (x86_64 Windows exe):
 *   x86_64-w64-mingw32-gcc -O2 -o sigill_bench.exe sigill_bench.c
 *   ili: x86_64-w64-mingw32-clang -O2 -o sigill_bench.exe sigill_bench.c
 *
 * Pokretanje: wine sigill_bench.exe [iteracija]   (podrazumevano 200000)
 * Izlaz: jedna "RESULT" linija po testu, laka za grep u CI-u.
 */
#include <windows.h>
#include <stdio.h>
#include <stdlib.h>

static volatile LONG64 hits;
static volatile ULONG64 first_rip, last_rip;
static volatile LONG phase;          /* broj tekuceg testa, za watchdog */
static volatile double phase_start;  /* us */
static double now_us(void);

static LONG CALLBACK veh(EXCEPTION_POINTERS *e)
{
    if (e->ExceptionRecord->ExceptionCode != EXCEPTION_ILLEGAL_INSTRUCTION)
        return EXCEPTION_CONTINUE_SEARCH;
    const BYTE *ip = (const BYTE *)e->ContextRecord->Rip;
    if (!first_rip) first_rip = (ULONG64)ip;
    last_rip = (ULONG64)ip;
    /* AAD ima imm8 (2 bajta), ostali su 1 bajt. */
    if (ip[0] == 0xd5) e->ContextRecord->Rip += 2;
    else e->ContextRecord->Rip += 1;
    hits++;
    return EXCEPTION_CONTINUE_EXECUTION;
}

struct test { const char *name; BYTE code[4]; int len; int faults; };

static const struct test tests[] = {
    { "nop (bez izuzetka, osnovna cena poziva)", { 0x90, 0xc3 }, 2, 0 },
    { "pusha 0x60", { 0x60, 0xc3 }, 2, 1 },
    { "daa 0x27",   { 0x27, 0xc3 }, 2, 1 },
    { "aad 0xd5",   { 0xd5, 0x0a, 0xc3 }, 3, 1 },
};

static double now_us(void)
{
    static LARGE_INTEGER f;
    LARGE_INTEGER c;
    if (!f.QuadPart) QueryPerformanceFrequency(&f);
    QueryPerformanceCounter(&c);
    return (double)c.QuadPart * 1e6 / (double)f.QuadPart;
}

static const char *machine_name(USHORT m)
{
    switch (m) {
    case IMAGE_FILE_MACHINE_AMD64: return "amd64";
    case IMAGE_FILE_MACHINE_ARM64: return "arm64";
    case IMAGE_FILE_MACHINE_I386: return "i386";
    case 0: return "none";
    default: return "other";
    }
}

/* Ako jedan poziv nikad ne vrati (npr. emulator ne pomeri Rip), javi stanje i izadji. */
static DWORD WINAPI watchdog(void *arg)
{
    (void)arg;
    for (;;) {
        Sleep(5000);
        double el = (now_us() - phase_start) / 1e6;
        if (el > 30) {
            printf("STUCK test=%ld posle %.0fs hits=%lld first_rip=%p last_rip=%p\n", (long)phase, el,
                   (long long)hits, (void *)first_rip, (void *)last_rip);
            fflush(stdout);
            ExitProcess(3);
        }
    }
}

int main(int argc, char **argv)
{
    long iters = argc > 1 ? atol(argv[1]) : 200000;
    if (iters < 1000) iters = 1000;

    /* Na cemu stvarno radimo: arm64 host znaci emulator (FEX), amd64 znaci Rosetta/nativno. */
    USHORT proc = 0, native = 0;
    BOOL (WINAPI *pIsWow64Process2)(HANDLE, USHORT *, USHORT *) =
        (void *)GetProcAddress(GetModuleHandleA("kernel32.dll"), "IsWow64Process2");
    if (pIsWow64Process2) pIsWow64Process2(GetCurrentProcess(), &proc, &native);
    printf("HOST native_machine=%s process_machine=%s iters=%ld\n",
           machine_name(native), machine_name(proc), iters);

    BYTE *page = VirtualAlloc(NULL, 4096 * 16, MEM_COMMIT | MEM_RESERVE, PAGE_EXECUTE_READWRITE);
    if (!page) { printf("ERROR VirtualAlloc %lu\n", GetLastError()); return 1; }
    if (!AddVectoredExceptionHandler(1, veh)) { printf("ERROR AddVectoredExceptionHandler\n"); return 1; }

    phase_start = now_us();
    CreateThread(NULL, 0, watchdog, NULL, 0, NULL);
    double base_us = 0;
    int failed = 0;
    for (size_t t = 0; t < sizeof(tests) / sizeof(tests[0]); t++) {
        /* Svaki test na svojoj 16K stranici (macOS arm64 ima 16K stranice). */
        BYTE *fn = page + t * 16384;
        memcpy(fn, tests[t].code, tests[t].len);
        FlushInstructionCache(GetCurrentProcess(), fn, tests[t].len);
        void (*call)(void) = (void (*)(void))fn;

        phase = (LONG)t; phase_start = now_us(); first_rip = last_rip = 0;
        printf("TEST %s fn=%p\n", tests[t].name, (void *)fn); fflush(stdout);
        hits = 0;
        call(); /* prvi poziv posebno: JIT + prvi izuzetak */
        printf("FIRST %s hits=%lld first_rip=%p us=%.1f\n", tests[t].name, (long long)hits,
               (void *)first_rip, now_us() - phase_start); fflush(stdout);
        for (int i = 0; i < 100; i++) call(); /* zagrijavanje */
        /* najvise `iters` poziva ili 5 s, sta pre dodje */
        hits = 0;
        double t0 = now_us();
        long done = 0;
        while (done < iters) {
            for (int k = 0; k < 64; k++) call();
            done += 64;
            if (now_us() - t0 > 5e6) break;
        }
        double dt = now_us() - t0;
        double per = dt / (double)done;
        if (!tests[t].faults) { base_us = per; }
        else if (hits != done) {
            printf("ERROR %s: ocekivano %ld izuzetaka, dobijeno %lld\n", tests[t].name, done, (long long)hits);
            failed = 1;
        }
        double exc = tests[t].faults ? per - base_us : 0;
        printf("RESULT test=\"%s\" calls=%ld us_per_call=%.3f us_per_exception=%.3f exceptions_per_s=%.0f\n",
               tests[t].name, done, per, exc, tests[t].faults && exc > 0 ? 1e6 / exc : 0.0);
        fflush(stdout);
    }
    /* AoE3 baca ~15-20k izuzetaka/s; koliko CPU-a bi to pojelo na ovom hostu. */
    printf("NOTE aoe3_load: 18000 izuzetaka/s * us_per_exception = %% jednog jezgra\n");
    return failed;
}
