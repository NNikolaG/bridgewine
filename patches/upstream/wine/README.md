# Wine patch-evi za upstream (WineHQ GitLab MR)

Gotove `git format-patch` verzije nasih ispravki, na Wine master-u (= 11.21,
2528aff). Ovo NIJE ono sto `bw` primenjuje; to je `patches/wine-11.21/`.

| Fajl | Sta | Proveren |
|---|---|---|
| 0001 | ntdll/tests: testovi za `\??\GLOBALROOT` i `\GLOBAL??` (`todo_wine`) | Linux x86_64: 9 todo pre patch-a |
| 0002 | ntdll: `\??\GLOBALROOT` i `\GLOBAL??\` prefiksi (+ skida `todo_wine`) | Linux x86_64: svih 9 prolazi; `file`, `path`, `om`, `directory` bez novih padova |
| 0003 | ntdll: vrati TEB GSBASE u `sigsys_handler()` na macOS-u | samo na Mac-u (kod je `#ifdef __APPLE__`); radi u BridgeWine runtime-u |

## Kako poslati
1. Fork `https://gitlab.winehq.org/wine/wine`, grana od `master`.
2. `git am patches/upstream/wine/000*.patch`
3. Proveri ime/email u `From:` i `Signed-off-by:` (sad: Nikola Gutic <nikola.gutic@outlook.com>).
4. Dva MR-a: GLOBALROOT (0001+0002) i GSBASE (0003). Odvojeno, jer ih gledaju razliciti ljudi.

## Tekst za MR
**ntdll: Resolve \??\GLOBALROOT and \GLOBAL??\ prefixes in file names.**
Applications build `\\.\GLOBALROOT\??\C:\...` names from
MemoryMappedFilenameInformation to reopen their own image; Wine returns
STATUS_OBJECT_NAME_NOT_FOUND. Seen with Age of Empires III: Definitive Edition.
Tests included (Windows behaviour needs a testbot run).

**ntdll: Restore the TEB GSBASE in sigsys_handler() on macOS.**
Direct syscalls trapped through SIGSYS (macOS 14+) return into the dispatcher
with GSBASE still on the pthread TSD, so the dispatcher reads a NULL TEB and
the syscall fails with STATUS_ACCESS_VIOLATION. Seen with AoE II/III DE copy
protection (NtAllocateVirtualMemory) on Apple Silicon under Rosetta 2.

## Otvoreno
- Testbot (Windows) za 0001 jos nije pokrenut; pokrece se sam na MR-u.
- Nisam mogao da pretrazim bugs.winehq.org odavde; pre slanja proveri da bag ne postoji.
- `patches/wine-11.21/0003` (CPU MHz pod Rosettom) ne ide upstream ovakav: bezuslovno
  `MaxMhz` bi i na Linuxu prijavio turbo umesto osnovne frekvencije. Upstream verzija
  mora da vazi samo kad TSC ne odgovara stvarnom CPU-u (Rosetta: TSC 1 GHz,
  `hw.cpufrequency_max` = 2400 MHz).
