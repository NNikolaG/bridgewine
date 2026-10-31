---
name: recipe-author
description: Kako se pise i menja BridgeWine recept za igru (recipes/<igra>.toml) - obavezna polja, vrednosti, testiranje, ocena kompatibilnosti. Koristi kad dodajes novu igru, menjas env/DLL override/engine u receptu ili upisujes rezultat testa. Use when adding or editing any game recipe TOML in BridgeWine.
---

# recipe-author

Recept = podaci, ne kod. Jedan fajl po igri: `recipes/<id>.toml`. Isti oblik za sve igre.

## Izvor istine
- Ako postoji `schema.json` ili parser u `src/` (serde struct): oni su zakon. Ovaj skill prati njih, ne obrnuto.
- Ako se razlikuju: popravi recept po kodu, javi razliku u PR-u.
- Uzor: `recipes/aoe3-de.toml`.

## Ime i id
- `id` = ime fajla bez `.toml`. Mala slova, crtice: `aoe3-de`, `aoe3-legacy`.
- Jedno izdanje igre = jedan recept. DE i original = dva recepta.

## Sekcije
```toml
# Jedna linija: sta je ovo, sta je potvrdjeno.
[game]
id = "aoe3-de"                 # obavezno
name = "Age of Empires III: Definitive Edition"   # obavezno
store = "steam"                # obavezno: steam (za sada samo steam)
steam_app_id = 933110          # obavezno ako store = steam
exe = "AoE3DE_s.exe"           # relativno od foldera igre

[runtime]
engine = "gptk"                # obavezno: gptk | dxmt | wined3d | dxvk-moltenvk
directx = 11                   # obavezno: 9 | 10 | 11 | 12
windows_version = "win10"      # win7 | win10 | win11

[env]                          # samo potvrdjene, string vrednosti
WINEMSYNC = "1"

[dll_overrides]                # ime = "n,b" | "b,n" | "n" | "b" | "" (iskljuci)
# d3d11 = "n,b"

[winetricks]
packages = []                  # npr. ["vcrun2019"]; samo ako bez njega puca

[known_issues]                 # kljuc = kratak opis; link na issue ako ima
multiplayer = "proveriti"

[tested]
chip = "M5"
macos = "26.x"
rating = "untested"            # obavezno
```

## Pravila
- Svaka vrednost u `[env]`, `[dll_overrides]`, `[winetricks]` ima komentar ZASTO ili link na belesku. Bez magije.
- Ne potvrdjeno = komentar `# proveriti`. Ne izmisljaj vrednosti.
- `steam_app_id`: sa store.steampowered.com/app/<id>, ne napamet.
- `exe`: tacno ime iz foldera igre, velika/mala slova bitna.
- Bez apsolutnih putanja, korisnickih imena, tokena.
- Bez putanja do GPTK ili fajlova igre u repou.
- Promena engine-a ili env-a = rezultat u `docs/notes/<igra>.md` pre izmene recepta.

## rating
| vrednost | znaci |
| --- | --- |
| `untested` | niko nije pokrenuo |
| `broken` | ne startuje ili ne moze da se igra |
| `playable` | igra se, ima problema (u `known_issues`) |
| `perfect` | radi kao na Windowsu, ukljucujuci multiplayer |

- `rating` menja samo neko ko je stvarno pokrenuo igru. Upisi i `chip`, `macos`.
- Cloud thread bez Maca: nikad ne podize `rating`.

## Provera pre commita
1. TOML se parsira: `python3 -c 'import sys,tomllib; tomllib.load(open(sys.argv[1],"rb"))' recipes/<id>.toml`
2. `id` == ime fajla.
3. Obavezna polja postoje (gore).
4. Kad postoji parser: `cargo test` (test ucitava sve `recipes/*.toml`). Nova igra = test prolazi bez izmene koda.
5. Kad postoji `schema.json`: validacija prolazi (CI radi isto).

## Commit
- Poruka: `recipe(<id>): <sta>`. Npr. `recipe(aoe3-de): engine dxmt, msync off`.
- Jedan recept po commitu kad moze.

## Povezano
- Env, engine, debug: skill `wine-gptk`.
