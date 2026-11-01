# Exception cost under Rosetta 2 and Wine

AoE3 DE's anti-tamper raises 14,000–19,000 `EXCEPTION_ILLEGAL_INSTRUCTION` per second
and handles them in its own vectored handler. These programs measure what one such
exception costs on Apple Silicon, and which part of that cost belongs to Wine.

| Program | Runs as | Measures |
| --- | --- | --- |
| `sigill.c` | macOS x86_64 (Rosetta) | `ud2` → SIGILL handler → return, no Wine |
| `tsdbench.c` | macOS x86_64 (Rosetta) | `_thread_set_tsd_base`, which Wine calls twice per signal |
| `vehbench.c` | Windows x86_64 (Wine) | `ud2` → vectored handler → continue |
| `vehsplit.c` | Windows x86_64 (Wine) | the same, split at VEH entry (rdtsc) |
| `ctxbench.c` | Windows x86_64 (Wine) | `RtlCaptureContext` + `NtContinue` alone |

```sh
clang -arch x86_64 -O2 -o sigill sigill.c && arch -x86_64 ./sigill 200000
clang -arch x86_64 -O2 -o tsdbench tsdbench.c && arch -x86_64 ./tsdbench 1000000
x86_64-w64-mingw32-gcc -O2 -o vehbench.exe vehbench.c && wine vehbench.exe 200000
x86_64-w64-mingw32-gcc -O2 -o vehsplit.exe vehsplit.c && wine vehsplit.exe 200000
x86_64-w64-mingw32-gcc -O2 -o ctxbench.exe ctxbench.c -lntdll && wine ctxbench.exe 200000
```

## Results

MacBook Pro M5, macOS 27.0, WineHQ 11.21 with the patches from `patches/wine-11.21`,
2026-11-01. The game was running in the background during some runs, so expect about
±10% noise.

| What | Cost per call |
| --- | --- |
| SIGILL round trip, plain macOS under Rosetta | 24–27 µs |
| … with `sigaltstack`/`SA_ONSTACK`, with AVX state live | 24–25 µs (no change) |
| `_thread_set_tsd_base` | 0.18 µs |
| Wine: `ud2` → VEH → continue | 28–33 µs |
| Wine: `ud2` → VEH entry | 27.1 µs |
| Wine: VEH return → resume (`NtContinue` path) | 0.76 µs |
| Wine: `RtlCaptureContext` + `NtContinue` | 0.41 µs |

About 90% of the cost is Rosetta 2 delivering the exception: a Mach exception goes to the
`com.apple.rosetta.exceptionserver` thread and then becomes a signal, and system time
dominates. Wine adds roughly 2–3 µs (two GSBASE switches, context save, KiUserExceptionDispatcher,
NtContinue), so shortening Wine's path cannot give a meaningful win. At 16,000 exceptions per
second the game spends about half a core on exception delivery alone. That is why it stalls
and its music buffer loops. The protection is not touched. The report for Apple is in
`docs/apple-feedback-rosetta-sigill.md`.
