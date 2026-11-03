# patches/arm64-fex

Eksperimentalni runtime bez Rosette: native arm64 Wine (ARM64EC) + FEX-Emu.
Build: `scripts/build-arm64-fex.sh` (samo macOS arm64). Mera: `scripts/bench-sigill.sh`.

## Poreklo (pinovano)

| Deo | Izvor | Licenca |
|---|---|---|
| Wine baza | [citi94/wine-macos-arm64](https://github.com/citi94/wine-macos-arm64) `macos-arm64-port` @ `0cc8848b` (Wine 11.10 + macOS arm64 port) | LGPL-2.1+ |
| `wine/0001-*.patch` | [Irz-Z/wine-macos-native](https://github.com/Irz-Z/wine-macos-native) v0.1.0 @ `594266db` | LGPL-2.1+ (`wine/COPYING.LIB`) |
| `fex/0001-*.patch`, `fex/0002-*.patch` | isto, za FEX-Emu tag `FEX-2609` | MIT (`fex/LICENSE-FEX-MIT`) |
| `fex/0003-*.patch` | nas (BridgeWine), ideja iz [FEX-darwin](https://github.com/FEX-Emu/FEX) `SupportsFaultFreeSynchronousFaults` | MIT |
| `llvm-mingw/0001-*.patch` | isto, za llvm-mingw 20260922 headere | mingw-w64 (javni domen / ZPL) |

Tudji patchevi su kopirani bez izmena. `fex/0003` je nas: nevazeci opkod (PUSHA, DAA,
AAD...) ne pravi vise host hlt/brk fault -> kernel signal -> Wine signal handler, nego
dispatcher odmah zove `SynchronousExceptionHandler` u ARM64EC modulu koji digne guest
izuzetak. To je put koji AoE3 zastita gadja ~18k puta u sekundi.

Sta rade ostali:

- TEB vise nije u x18 (macOS ga brise pri svakom povratku iz kernela), nego u pthread TSD slotu 700.
- `WINEARM64ECEMU=1`: wineserver prihvata AMD64 exe kad je ARM64EC emulator (FEX) podesen.
- `KUSER_SHARED_DATA` na 0x17ffe0000 i za ARM64EC kod.

Napomena autora originala: patchevi su pisani uz pomoc AI-ja; FEX upstream ne prima
AI kod, zato `fex/` patcheve ne saljemo upstream.

Poznata ogranicenja: nema 32-bitnog x86 (WoW64) jos; x64 kod koji direktno cita
0x7ffe0000 (KUSER_SHARED_DATA) ce dobiti access violation (FEX-darwin to emulira,
mi jos ne). Ovo je prvo za merenje cene izuzetaka, ne jos za igranje.
