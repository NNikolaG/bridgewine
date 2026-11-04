# #UD exceptions: Rosetta 2 vs arm64 Wine + FEX

AoE3 DE's protection raises ~16,000 `EXCEPTION_ILLEGAL_INSTRUCTION`/s by executing
opcodes that are invalid in 64-bit mode (PUSHA, DAA, AAD). Under Rosetta each one costs
~27 µs (see `../rosetta-exceptions` on branch `rosetta-exception-bench`), which stalls the
game. Here we measure the same exceptions on a native arm64 Wine where FEX translates
x86_64 in-process (ARM64EC), so the trap is a native arm64 one, not Rosetta's
exception server.

| File | Runs as | Measures |
| --- | --- | --- |
| `opbench.c` / `opbench.exe` | Windows x86_64 | `ud2`, `pusha`, `daa`, `aad` → VEH → continue; checks the exception lands on the instruction |
| `sigill_arm64.c` | macOS arm64 | native `udf` → SIGILL → return (the floor) |
| `run.sh` | macOS | all three: native floor, Rosetta+WineHQ, arm64 Wine+FEX |

```sh
sh bench/fex-exceptions/run.sh 100000
```

`opbench.exe` is built with `x86_64-w64-mingw32-gcc -O2 opbench.c` (mingw-w64 GCC 13) and
stripped. Sanity run under Wine 9 on x86_64 Linux: all four ops `OK`.

Goal: ≤5 µs per exception (≤8% of a core at 16k/s). If FEX gets there, the next step is
running Steam + AoE3 on this runtime; if it reports `BAD`, FEX decodes these opcodes
wrongly and the game would break regardless of speed.

Runtime: `bw runtime install-arm64` (Irz-Z/wine-macos-native v0.1.0: Wine 11.10 arm64,
LGPL; FEX-2609, MIT; sha256-checked). It has no 32-bit x86 support yet and no
DXVK/D3DMetal, so it is only for this measurement for now.

## Results (MacBook Pro M5, 2026-11-04)

| What | per exception |
| --- | --- |
| native arm64 SIGILL | 1.59 µs |
| Rosetta + WineHQ 11.21 (ud2/pusha/daa/aad) | 18.1–19.6 µs |
| arm64 Wine 11.10 + FEX (all four) | 3.1–3.5 µs, all `OK` |

FEX is ~6x faster than Rosetta. Its stock DLL hung at 100% CPU after ~170–200k
exceptions: native code calls the guest VEH without pushing a FEX call-ret stack entry,
so every handler `ret` pops one, and the stack overflows after 3 MiB / 16 B = 196,608
exceptions. FEX's 4K guard pages don't exist on macOS's 16K pages, so the overflow is
never caught. `patches/fex-2609/0003` makes the guards whole host pages, `0004` resets
the call-ret stack on every guest exception. Built by `.github/workflows/fex-arm64ec.yml`;
`bw runtime install-arm64` swaps it in (original kept as `.orig`).
