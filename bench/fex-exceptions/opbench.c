/* Cost of the #UD exceptions AoE3 DE's protection raises, handled by a vectored
 * handler (EXCEPTION_CONTINUE_EXECUTION). Same program for Rosetta+Wine and
 * arm64 Wine+FEX, so the numbers compare directly.
 *
 *   opbench.exe [count] [op]     op = ud2 | pusha | daa | aad | all (default)
 *
 * PUSHA (60), DAA (27) and AAD (D5 0A) are invalid in 64-bit mode and must
 * raise EXCEPTION_ILLEGAL_INSTRUCTION at the instruction itself. The handler
 * checks that, so a translator that decodes them wrongly shows up as "BAD". */
#include <windows.h>
#include <stdio.h>
#include <string.h>

static volatile LONG hits, bad;
static DWORD last_code;

static LONG CALLBACK handler(EXCEPTION_POINTERS *ep)
{
    const unsigned char *p = (const unsigned char *)ep->ContextRecord->Rip;
    last_code = ep->ExceptionRecord->ExceptionCode;
    if (last_code != EXCEPTION_ILLEGAL_INSTRUCTION)
        return EXCEPTION_CONTINUE_SEARCH;
    if (p[0] == 0x0f && p[1] == 0x0b) ep->ContextRecord->Rip += 2;       /* ud2 */
    else if (p[0] == 0x60 || p[0] == 0x27) ep->ContextRecord->Rip += 1;  /* pusha, daa */
    else if (p[0] == 0xd5) ep->ContextRecord->Rip += 2;                  /* aad imm8 */
    else { bad++; return EXCEPTION_CONTINUE_SEARCH; }
    hits++;
    return EXCEPTION_CONTINUE_EXECUTION;
}

static void op_ud2(void)   { __asm__ volatile("ud2"); }
static void op_pusha(void) { __asm__ volatile(".byte 0x60"); }
static void op_daa(void)   { __asm__ volatile(".byte 0x27"); }
static void op_aad(void)   { __asm__ volatile(".byte 0xd5, 0x0a"); }

static int run(const char *name, void (*op)(void), int n)
{
    LARGE_INTEGER f, a, b;
    hits = bad = 0;
    QueryPerformanceFrequency(&f);
    QueryPerformanceCounter(&a);
    for (int i = 0; i < n; i++) op();
    QueryPerformanceCounter(&b);
    double us = (double)(b.QuadPart - a.QuadPart) * 1e6 / f.QuadPart / n;
    int ok = hits == n && !bad;
    printf("%-6s %8d  %7.2f us  %8.0f /s/thread  %s\n", name, n, us, 1e6 / us,
           ok ? "OK" : "BAD");
    fflush(stdout);
    return ok;
}

int main(int argc, char **argv)
{
    int n = argc > 1 ? atoi(argv[1]) : 100000;
    const char *which = argc > 2 ? argv[2] : "all";
    static const struct { const char *name; void (*op)(void); } ops[] = {
        { "ud2", op_ud2 }, { "pusha", op_pusha }, { "daa", op_daa }, { "aad", op_aad },
    };
    int ok = 1;
    AddVectoredExceptionHandler(1, handler);
    printf("op        count  per exception  rate            check\n");
    for (unsigned i = 0; i < sizeof(ops) / sizeof(ops[0]); i++)
        if (!strcmp(which, "all") || !strcmp(which, ops[i].name))
            ok &= run(ops[i].name, ops[i].op, n);
    printf("AoE3 needs ~16000/s; at <=5 us that is <=8%% of one core.\n");
    return ok ? 0 : 1;
}
