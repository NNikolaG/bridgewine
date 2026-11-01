# AoE3 DE na Macu preko Apple Game Porting Toolkita (faza 0)

BridgeWine je besplatna alternativa CrossOveru. Ovde se koristi Appleov Wine sa D3DMetal-om (GPTK), pokretan iz Terminala.
Ovo je put koji ce `bw` kasnije automatizovati, pa je vazno da svaki korak
zapises u [docs/notes/aoe3.md](notes/aoe3.md).

> Upozorenje: GPTK je Appleov alat za programere, ne za igrace. Nema GUI, nema
> podrske, i komande ispod mogu da se razlikuju izmedju verzija. Gde pise
> "proveri", uporedi sa README fajlom iz verzije koju si skinuo.

## Ukratko

| Stavka | Preporuka |
| --- | --- |
| macOS | 15 Sequoia ili noviji (GPTK 3 trazi bar 14 Sonoma) |
| GPTK | 3.x, prebuilt od Gcenxa preko Homebrewa |
| Prefiks | `~/Games/bw-aoe3`, Windows 10 |
| Grafika | D3DMetal (jedino sto GPTK ima), DXVK kao poseban eksperiment |
| Sync | `WINEMSYNC=1` (ili `WINEESYNC=1` ako MSync nije podrzan) |

Bitno: na CrossOveru je D3DMetal brisao tekst sa dugmica u meniju AoE3 DE
(AppleGamingWiki). GPTK koristi isti D3DMetal, pa ocekuj isti problem. Zato je
ovaj test pre svega poredjenje: da li noviji D3DMetal (3.x) to popravlja.

## Sta ti treba

- M5 MacBook Pro, bar 16 GB RAM-a.
- Besplatan Apple Developer nalog (za skidanje GPTK-a sa
  <https://developer.apple.com/download/all/?q=game%20porting%20toolkit>).
- Steam nalog sa AoE3 DE (Steam App ID 933110).
- Oko 60 GB slobodnog prostora.

## Koraci

### 1. Rosetta 2

```sh
softwareupdate --install-rosetta --agree-to-license
```

### 2. Homebrew

Ako vec imas Homebrew (`brew --version` radi), preskoci. Inace:

```sh
/bin/bash -c "$(curl -fsSL https://raw.githubusercontent.com/Homebrew/install/HEAD/install.sh)"
```

Stari vodici traze posebnu x86_64 verziju Homebrewa u `/usr/local` jer su GPTK
kompajlirali iz izvornog koda (preko sat vremena). Sa prebuilt paketom od Gcenxa
to ne treba, obican ARM Homebrew je dovoljan.

### 3. Instaliraj GPTK (prebuilt)

```sh
brew tap gcenx/wine
brew install --cask gcenx/wine/game-porting-toolkit
xattr -dr com.apple.quarantine "/Applications/Game Porting Toolkit.app"
```

Novije verzije Homebrewa vise nemaju opciju `--no-quarantine`, pa se karantin
skida rucno sa `xattr`. Bez toga macOS blokira Wine pri prvom pokretanju.

Ako dobijes `/opt/homebrew is not writable`, vrati vlasnistvo sebi sa
`sudo chown -R $(whoami) /opt/homebrew`. Nikad ne pokreci `sudo brew`.

Proveri gde je Wine zavrsio i zapamti putanju:

```sh
ls "/Applications/Game Porting Toolkit.app/Contents/Resources/wine/bin/"
export GPTK_WINE="/Applications/Game Porting Toolkit.app/Contents/Resources/wine/bin/wine64"
"$GPTK_WINE" --version
```

Ako putanja nije ista, nadji je sa `brew info --cask game-porting-toolkit` i
ispravi `GPTK_WINE`. Upisi verziju u dnevnik.

### 4. D3DMetal iz Appleovog paketa (proveri)

Gcenxov paket obicno vec ima D3DMetal. Proveri:

```sh
ls "/Applications/Game Porting Toolkit.app/Contents/Resources/wine/lib/external/"
```

Ako tu nema `D3DMetal.framework`, skini **Game Porting Toolkit** dmg sa Apple
Developer sajta, otvori ga i kopiraj `redist/lib/` preko Wine `lib` foldera:

```sh
ditto "/Volumes/Evaluation environment for Windows games 3.0/redist/lib/" \
  "/Applications/Game Porting Toolkit.app/Contents/Resources/wine/lib/"
```

Tacno ime volumena zavisi od verzije, vidi ga sa `ls /Volumes`.

### 5. Napravi prefiks

```sh
export WINEPREFIX="$HOME/Games/bw-aoe3"
mkdir -p "$WINEPREFIX"
"$GPTK_WINE" winecfg
```

U prozoru `winecfg` postavi **Windows Version** na **Windows 10**, klikni
**Apply** pa **OK**.

### 6. Instaliraj Steam

```sh
curl -L -o ~/Downloads/SteamSetup.exe \
  https://cdn.cloudflare.steamstatic.com/client/installer/SteamSetup.exe
WINEMSYNC=1 "$GPTK_WINE" ~/Downloads/SteamSetup.exe
```

Prodji kroz instalaciju sa podrazumevanim opcijama. Zatim pokreni Steam:

```sh
WINEMSYNC=1 MTL_HUD_ENABLED=1 "$GPTK_WINE" \
  'C:\Program Files (x86)\Steam\steam.exe'
```

Ako je Steam prozor crn ili prazan:

- zatvori sve sa `"$(dirname "$GPTK_WINE")/wineserver" -k`
  i pokreni ponovo iz novog Terminal prozora;
- ili se uloguj u macOS Steam pa kopiraj `config`, `registry.vdf` i `userdata`
  u `"$WINEPREFIX/drive_c/Program Files (x86)/Steam/"` (AppleGamingWiki).

### 7. Instaliraj i pokreni AoE3 DE

1. U Steamu instaliraj **Age of Empires III: Definitive Edition**.
2. Pokreni je kroz Steam (**Play**), ili direktno:

   ```sh
   cd "$WINEPREFIX/drive_c/Program Files (x86)/Steam/steamapps/common/AoE3DE"
   WINEMSYNC=1 MTL_HUD_ENABLED=1 "$GPTK_WINE" AoE3DE_s.exe
   ```

   Ime foldera i exe-a proveri sa `ls`, pa ispravi
   [recipes/aoe3-de.toml](../recipes/aoe3-de.toml) ako se razlikuje.
3. Metal HUD (gore desno) pokazuje FPS i da li ide preko D3DMetal-a.

### 8. Sta da proveris

- Da li meni ima tekst na dugmicima. Ako nema, to je poznati D3DMetal problem.
- Da li se mapa crta ispravno (bele pruge na neistrazenom delu i obojene ivice
  na okeanu su prijavljene na Macu, resava ih promena grafickih podesavanja).
- FPS na 1080p i native, ista skirmish mapa, 5 minuta.
- Za 4+ igraca probaj Quality na Medium i FPS ogranicen na 30.

### 9. (Opciono) DXVK umesto D3DMetal

Ako D3DMetal brise tekst, sledeci eksperiment je DXVK preko MoltenVK-a
(macOS fork od Gcenxa, <https://github.com/Gcenx/DXVK-macOS>). Grubo:

1. Skini release i kopiraj `x64/*.dll` u `"$WINEPREFIX/drive_c/windows/system32/"`,
   a `x32/*.dll` u `syswow64`.
2. Pokreni sa override-om:

   ```sh
   WINEDLLOVERRIDES="d3d11,dxgi=n,b" WINEMSYNC=1 "$GPTK_WINE" AoE3DE_s.exe
   ```

Ovo nije provereno na AoE3 DE sa GPTK-om. Ako ne radi, ukloni override i vrati
se na D3DMetal. Rezultat (radi ili ne) upisi u dnevnik, jer ce `bw` recept
`graphics = "dxvk"` zavisiti od toga.

## Pomocna funkcija

Da ne kucas putanje svaki put, dodaj u `~/.zshrc`:

```sh
export GPTK_WINE="/Applications/Game Porting Toolkit.app/Contents/Resources/wine/bin/wine64"
aoe3() {
  WINEPREFIX="$HOME/Games/bw-aoe3" WINEMSYNC=1 MTL_HUD_ENABLED=1 \
    "$GPTK_WINE" 'C:\Program Files (x86)\Steam\steam.exe' -applaunch 933110
}
```

Ovo je u sustini ono sto ce `bw run aoe3-de` raditi.

## Poznati problemi

| Problem | Resenje | Izvor |
| --- | --- | --- |
| Nema teksta na dugmicima u meniju (D3DMetal) | probaj DXVK (korak 9) | AppleGamingWiki |
| Steam crn ili prazan | ponovo pokreni iz novog Terminala, ili kopiraj login iz macOS Steama | AppleGamingWiki (GPTK strana) |
| Bele pruge i obojene ivice na mapi | promeni graficka podesavanja u igri | AppleGamingWiki |
| Online multiplayer nepouzdan | nema resenja, testiraj i zapisi | izvestaji korisnika |
| Komande se razlikuju od ovog uputstva | gledaj README iz svoje GPTK verzije | Apple GPTK README |

## Izvori

- AppleGamingWiki, Game Porting Toolkit:
  <https://applegamingwiki.com/wiki/Game_Porting_Toolkit>
- AppleGamingWiki, Age of Empires III: Definitive Edition:
  <https://applegamingwiki.com/wiki/Age_of_Empire_III:_Definitive_Edition>
- Gcenx prebuilt GPTK 3.0 (zahtevi: Apple Silicon, macOS 14+, 16 GB RAM):
  <https://github.com/Gcenx/game-porting-toolkit/releases/tag/Game-Porting-Toolkit-3.0-2>
- Apple Developer, preuzimanje GPTK-a:
  <https://developer.apple.com/games/game-porting-toolkit/>
- CodeWeavers kompatibilnost, AoE3 DE (samo za poredjenje):
  <https://codeweavers.com/compatibility/crossover/age-of-empires-iii-definitive-edition>
