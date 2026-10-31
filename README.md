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

## AoE3 DE za tri komande

```sh
bw bottle create aoe3-de   # Wine prefiks u ~/Games/BridgeWine/bottles/aoe3-de
bw steam install aoe3-de   # skida i tiho instalira Windows Steam
bw run aoe3-de             # pokrece igru kroz Steam (-applaunch 933110)
```

Svaka komanda prima `--dry-run` da samo ispise sta bi pokrenula. `BW_HOME`
menja gde zive bottle-i, a `BW_WINE` koji Wine se koristi.

## Roadmap

Faze: 0 rucni proboj, 1 skripta koja to ponavlja, 2 recepti kao podaci,
3 baza kompatibilnosti, 4 GUI launcher, 5 odrzavanje.

## Licenca

MIT, vidi [LICENSE](LICENSE).
