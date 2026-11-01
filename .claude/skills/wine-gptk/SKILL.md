---
name: wine-gptk
description: Pravila za Wine + Apple Game Porting Toolkit (D3DMetal), DXMT i Rosetta 2 na Apple Silicon Macu. Koristi kad pravis ili debagujes bottle (WINEPREFIX), pokreces Steam ili igru, biras graficki prevodilac, podesavas env promenljive (MSync, Metal HUD, WINEDEBUG) ili citas Wine logove. Use for any Wine/GPTK/D3DMetal/DXMT/bottle/prefix work in BridgeWine.
---

# wine-gptk

Kratko. Komande i putanje doslovno. Sve sto nije potvrdjeno na M5 = "proveriti", upisi u `docs/notes/<igra>.md`.

## Cilj projekta
- BridgeWine = besplatna zamena za CrossOver (i Whisky). Sve radimo sami: Wine + GPTK + Rosetta.
- Nikad ne trazi od korisnika da instalira ili proba CrossOver, Whisky, Heroic ili sl. kao korak. Ne ni "za poredjenje".
- Ako fali korak: napisi ga u `bw` ili u beleske, ne salji korisnika na tudji alat.

## Pravni okvir (uvek)
- Ne commitovati: fajlove igara, GPTK/D3DMetal binarne fajlove, Steam, krekove.
- GPTK korisnik skida sam (Apple Developer). Mi samo trazimo putanju.
- Bez zaobilazenja DRM-a i anti-cheata.

## Preduslovi (`bw doctor` proverava isto)
- Apple Silicon: `uname -m` = `arm64`.
- macOS verzija: `sw_vers -productVersion`.
- Rosetta 2: `arch -x86_64 /usr/bin/true` uspe. Ako ne: `softwareupdate --install-rosetta --agree-to-license`.
- Wine je x86_64 build, ide kroz Rosettu. Homebrew alati za njega: `arch -x86_64 /usr/local/bin/brew`.
- GPTK: D3DMetal ide u `lib/external/` Wine builda (`D3DMetal.framework`, `libd3dshared.dylib`). Proveri da postoje pre pokretanja.

## Bottle = jedan WINEPREFIX po igri
- Putanja: `~/Library/Application Support/BridgeWine/bottles/<igra-id>` (id iz recepta).
- Nikad deljen prefiks izmedju igara. Nikad `~/.wine`.
- Uvek eksplicitno: `WINEPREFIX=<bottle>` u svakoj komandi. Bez globalnog export-a u skriptama.
- Napravi: `WINEPREFIX=<bottle> wine wineboot --init`.
- Windows verzija iz recepta (`runtime.windows_version`): `WINEPREFIX=<bottle> winecfg -v win10`.
- Ugasi sve u bottle-u: `WINEPREFIX=<bottle> wineserver -k`. Uradi pre brisanja ili promene DLL-ova.
- Obrisi = obrisi folder. Pitaj korisnika pre brisanja (Steam igre su unutra, desetine GB).

## Graficki prevodioci (`runtime.engine`)
| engine | za | kako |
| --- | --- | --- |
| `gptk` | DX11, DX12 | D3DMetal iz GPTK, default za AoE3 DE |
| `dxmt` | DX10, DX11 | DXMT DLL-ovi (`d3d11`, `dxgi`) u bottle + override `n,b` |
| `wined3d` | DX9 i stariji | ugradjeni Wine, OpenGL; za AoE3 legacy |
| `dxvk-moltenvk` | DX9-11 | rezerva, Vulkan preko MoltenVK |

- Menjas engine = nova proba, novi red u tabeli u beleskama (FPS 1080p, FPS native, greske).
- DLL override format: `WINEDLLOVERRIDES="d3d11,dxgi=n,b"`. U receptu ide u `[dll_overrides]`.

## Env promenljive
- `WINEMSYNC=1`: brza sinhronizacija (Mach semafori). Default ukljuceno. Ako igra visi, proba sa `0`.
- `WINEESYNC=1`: starija alternativa, ne oba odjednom.
- `MTL_HUD_ENABLED=1`: Metal HUD (FPS, GPU vreme). Za merenje. U receptu default `0`.
- `WINEDEBUG=-all`: tiho, za normalno igranje.
- `WINEDEBUG=+loaddll,+seh`: koji DLL se ucitao, izuzeci. Za crash.
- `WINEDEBUG=+d3d,+dxgi`: grafika. Ogromni logovi, kratko pokretanje.
- `ROSETTA_ADVERTISE_AVX=1`: igra trazi AVX. Proveriti da li treba AoE3 DE.
- Nove env promenljive: prvo u belesku sa razlogom, tek posle potvrde u recept `[env]`.

## Steam u bottle-u
- Windows Steam instaler u bottle: `WINEPREFIX=<bottle> wine SteamSetup.exe`.
- Putanja: `<bottle>/drive_c/Program Files (x86)/Steam/steam.exe`.
- Crn/prazan Steam prozor = steamwebhelper (CEF) crta u posebnom GPU procesu, winemac.drv to ne prikazuje (Wine bug 56014). Resava nas kernelbase patch (`patches/wine-11.21/0004-*`): Wine sam dodaje `--disable-gpu --single-process` kad Steam pokrene helper. `BW_STEAMWEBHELPER_ARGS` menja te flagove (prazno = iskljuceno). Ne menjati Steam fajlove.
- Login, kupovina, DRM: radi korisnik, ne mi.
- Igra pokrenuta direktno preko exe iz recepta (`game.exe`) ili `steam.exe -applaunch <steam_app_id>`.

## Logovi
- Uvek preusmeri: `... 2> "$BOTTLE/logs/$(date +%Y%m%d-%H%M%S).log"`.
- Igre pisu svoje logove u `drive_c/users/<user>/AppData/` ili `Documents/My Games/`. Proveri tamo kad Wine log cist.
- macOS crash izvestaj: `~/Library/Logs/DiagnosticReports/` (`wine64-preloader`, `wine`).
- U belesku: samo relevantne linije (prvih ~20 oko `err:` / `fixme:` koji se ponavlja). Ne ceo log.

## Debug redosled
1. `wineserver -k`, ponovi. Prolazni problem?
2. `WINEDEBUG=+loaddll,+seh`. Fali DLL? Krahira gde?
3. Promeni engine (gptk -> dxmt -> wined3d). Problem nestane = prevodilac.
4. `WINEMSYNC=0`. Nestane = sync.
5. Novi cist bottle. Nestane = pokvaren prefiks.
6. Trazi poznat bag: Wine bugzilla, GPTK/DXMT issues, AppleGamingWiki. Link u belesku.
7. Sve sto si probao + ishod u `docs/notes/<igra>.md` sekciju Problemi. Ne brisi neuspele pokusaje.

## Gde se radi
- Cloud thread nema Mac: samo kod, recepti, beleske. Ne tvrdi da nesto radi.
- Pravo pokretanje igre, GPTK, FPS merenje: korisnikov Mac preko Remote Control.

## Povezano
- Recepti: skill `recipe-author`.
- Patch za Wine/DXMT: skill `wine-patch-upstream`.
