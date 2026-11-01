# BridgeWine

Most izmedju Windows igara i Apple Silicon Maca: besplatna, open-source alternativa CrossOveru. BridgeWine automatizuje Wine,
Apple Game Porting Toolkit (D3DMetal) i Rosetta 2 tako da se igra pokrene
jednom komandom. Prva ciljna igra je **Age of Empires III** sa Steama na M5 MacBook Pro.

> Status: faza 0 (rucno pokretanje i beleske). Jos nista ne radi automatski.

## Kako radi

- `bw` je CLI napisan u Rustu koji pravi izolovan Wine prefiks (**bottle**) po igri.
- Svaka igra ima **recept** u `recipes/<igra>.toml`: grafika (D3DMetal, DXMT, WineD3D),
  env promenljive, DLL override-i i poznati problemi.
- Kasnije: SwiftUI launcher koji poziva isti CLI.

## Sta BridgeWine ne radi

Ne distribuira fajlove igara, Appleove GPTK binarne fajlove, krekove niti zaobilazi DRM.
Igru kupujes i instaliras preko svog Steam naloga, a GPTK skidas sam sa Apple Developer sajta.

## Build i prve komande

```sh
cargo build --release
./target/release/bw doctor              # sta fali na Macu
./target/release/bw recipe list         # sve igre
./target/release/bw recipe show aoe3-de # recept za AoE3 DE
./target/release/bw recipe check        # validacija (radi i CI)
```

## AoE3 DE jednom komandom

```sh
bw setup aoe3-de    # WineHQ runtime, nasi Wine moduli, DXMT, bottle, Windows Steam
bw run aoe3-de      # Steam (uloguj se, prvi put instaliraj igru), pa igra
bw doctor aoe3-de   # sta fali: karantin, runtime, moduli, DXMT, bottle, Steam, wrapper, igra
```

`bw setup` preskace sve sto vec postoji, pa je bezbedno pokrenuti ga ponovo (npr. posle
nove verzije `bw`-a ili patch-eva). Recept kaze sta treba: `patched = true` za nase Wine
module, `graphics = "dxmt"`, `[steam] cef_workaround = "wrapper"` za crn Steam prozor.

## AoE3 DE korak po korak


```sh
bw runtime install         # WineHQ Wine u ~/Games/BridgeWine/runtimes (64-bitni Steam ne radi na GPTK Wine 7.7)
bw runtime install-patched # nasi Wine moduli (patches/) sa GitHub release-a; bez kompajliranja
# ili sam: bw runtime build-patched (treba brew install bison flex mingw-w64)
bw bottle create aoe3-de   # Wine prefiks u ~/Games/BridgeWine/bottles/aoe3-de
bw steam install aoe3-de   # skida i tiho instalira Windows Steam
bw run aoe3-de             # pokrece igru kroz Steam (-applaunch 933110)
bw run aoe3-de --graphics dxvk   # DXVK (Vulkan/MoltenVK) umesto DXMT-a, npr. kad je tekst u meniju isecen
```

Ako Steam javi "Steam needs to be online" ili "http error 0", bottle nije
potpuno inicijalizovan: `bw bottle repair aoe3-de`. WPAD zastoj (`wpad.local` DNS upit
koji visi ~135 s) `bw run` i `bw steam install` sami zaobilaze praznim DNS domenom u bottle-u. Sve Wine procese gasi
`bw bottle kill aoe3-de`.

Svaka komanda prima `--dry-run` da samo ispise sta bi pokrenula. `BW_HOME`
menja gde zive bottle-i, a `BW_WINE` koji Wine se koristi.

## Roadmap

Faze: 0 rucni proboj, 1 skripta koja to ponavlja, 2 recepti kao podaci,
3 baza kompatibilnosti, 4 GUI launcher, 5 odrzavanje.

## Licenca

MIT, vidi [LICENSE](LICENSE).
