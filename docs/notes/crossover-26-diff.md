# CrossOver 26.3 vs upstream Wine: sta CrossOver radi drugacije (2026-11-02)

Izvor: CodeWeavers-ov objavljeni izvorni kod `crossover-sources-26.3.0.tar.gz`
(LGPL-2.1+, https://www.codeweavers.com/crossover/source; git kopija:
github.com/gilnoh/winecx-sources, tag `winecx-26.3.0`). CrossOver 26.3 = **Wine 11.0**.
Uporedjeno sa `wine-11.0` (WineHQ), pa preneseno na nas `wine-11.21`.
CrossOver je samo referenca; nista se ne instalira niti koristi iz njega.

## Zvuk
`dsound`, `winecoreaudio.drv`, `mmdevapi`: **nema bitnih razlika**. CrossOver ima samo
Fallout 4 hack (Sleep(100) u DirectSoundDevice_Release) i dozvolu za mikrofon. Petlja
muzike u AoE3 je zastoj igre (vidi `aoe3.md`), ne bag u audio kodu, i CrossOver je ne
resava posebnim audio kodom.

## Izuzeci pod Rosettom (preneseno)
| Patch | CW hack | Sta radi |
| --- | --- | --- |
| 0012 | 24256, 24265 | Rosetta u signal kontekstu daje pogresan MXCSR; Wine ga je vracao posle svakog izuzetka (gubi FTZ/DAZ i rounding programa, spora denormal matematika). Sada se cita pravi registar. |
| 0013 | 20186, 23427 | XGETBV (XCR0) i CET RDSSP se emuliraju u handleru, bez punog izuzetka u program. `WINEDEBUG=warn+seh` loguje bajtove prvih 32 razlicitih ilegalnih instrukcija. |
| 0014 | 22131 | Postavljanje debug registara pod Rosettom vraca uspeh umesto STATUS_UNSUCCESSFUL. |
| 0015 | 24945, 25719 | Lazne write/exec greske na W|X i X stranicama: prebaci exec bit i nastavi, umesto access violation. |

CrossOver drugacije postavlja GSBASE (ne menja ga pri svakom signalu/syscall-u), ali
`_thread_set_tsd_base` kosta samo 0.18 us (bench u PR #18), pa to nismo prenosili.

**Bitno:** ako zastita AoE3 namerno izvrsava `ud2`, ni CrossOver to ne ubrzava:
cenu (~25 us) placa Rosetta pre nego sto Wine dobije signal. Sikarugir (koji koristi
CrossOver-ov Wine) ima isti problem sa AoE2 DE (#233). Log iz 0013 kaze koja je
instrukcija; ako je XGETBV, 0013 uklanja te izuzetke potpuno.

## Performanse: msync (nije jos preneseno)
CrossOver ima **msync** (Mach semafori + `__ulock_wait`, Marc-Aurel Zent): cekanja na
event/mutex/semafor ne idu preko wineservera. Upstream na macOS-u nema ntsync, pa svako
cekanje ide kroz wineserver. Ukljucuje se sa `WINEMSYNC=1`. Port na 11.21:
`dlls/ntdll/unix/msync.c` (~1160 linija), `server/msync.c` (~820), izmene u
`sync.c`, `server/inproc_sync.c`, `server/thread.c`, protocol.def; trazi i zamenu
`wineserver` binarnog fajla u runtime-u. Sledeci korak ako 0012-0015 ne budu dovoljni.

## Ostalo (nije za AoE3)
wow64cpu mode switch pod Rosettom (32-bit), `WINENCPU`, force LAA, ASLR pre Viste.
