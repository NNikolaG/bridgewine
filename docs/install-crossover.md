# AoE3 DE na Macu preko CrossOvera (faza 0)

Ovo je najlaksi put da Age of Empires III: Definitive Edition proradi na M5
MacBook Pro. CrossOver je placen (ima besplatan probni period od 14 dana), ali sve
podesava kroz GUI. Rezultate upisuj u [docs/notes/aoe3.md](notes/aoe3.md).

Ako hoces besplatnu varijantu sa Appleovim alatom, vidi
[install-gptk.md](install-gptk.md).

## Ukratko

| Stavka | Preporuka |
| --- | --- |
| CrossOver | najnoviji (26.x u trenutku pisanja) |
| Bottle | Windows 10 64-bit, samo za Steam + AoE3 |
| Grafika | **DXVK**, ne D3DMetal |
| MSync | ukljucen |
| Multiplayer | nepouzdan, testiraj posebno |

Zasto DXVK: sa D3DMetal-om nestaje tekst na dugmicima u glavnom meniju i
podesavanjima. Sa DXVK-om se tekst vidi, a performanse su i dalje dobre
(prijava: M1 Pro, CrossOver 24, 2560x1440 na 60 FPS). Izvor: AppleGamingWiki.

## Sta ti treba

- M5 MacBook Pro sa azurnim macOS-om.
- Steam nalog na kom je AoE3 DE (Steam App ID 933110).
- Oko 60 GB slobodnog prostora (igra plus Steam plus bottle).
- Rosetta 2. CrossOver je obicno instalira sam pri prvom pokretanju, a mozes i rucno:

  ```sh
  softwareupdate --install-rosetta --agree-to-license
  ```

## Koraci

### 1. Instaliraj CrossOver

1. Skini CrossOver sa <https://www.codeweavers.com/crossover>.
2. Raspakuj i prevuci `CrossOver.app` u `Applications`.
3. Pokreni ga i izaberi probni period ili unesi licencu.
4. Zapisi verziju (CrossOver > About CrossOver) u dnevnik.

### 2. Napravi bottle i instaliraj Steam

1. U CrossOveru klikni **Install** (ili **Install a Windows Application**).
2. U pretrazi ukucaj `Steam` i izaberi zvanicni Steam iz kataloga.
3. Kada pita za bottle, izaberi **New bottle**, nazovi ga `aoe3` i ostavi tip
   **Windows 10 64-bit**.
4. Klikni **Install** i sacekaj da CrossOver skine i instalira Steam.
5. Pokreni Steam iz bottle-a i uloguj se. Pusti ga da se azurira do kraja.

### 3. Podesi grafiku na DXVK

1. Zatvori Steam.
2. U levoj koloni izaberi bottle `aoe3`, pa otvori **Bottle Settings**
   (ikona sa zupcanikom ili desni klik > Settings).
3. Pod **Graphics** (u starijim verzijama **Advanced Settings**) izaberi **DXVK**.
   Ako postoji poseban prekidac **D3DMetal**, iskljuci ga.
4. Ukljuci **MSync** (u starijim verzijama **ESync**).
5. Ostalo ostavi na podrazumevanom.

### 4. Instaliraj i pokreni AoE3 DE

1. Pokreni Steam iz bottle-a `aoe3`.
2. U biblioteci nadji **Age of Empires III: Definitive Edition** i klikni **Install**.
   Lokaciju ostavi na `C:\Program Files (x86)\Steam`.
3. Kada se skine, klikni **Play**. Prvo pokretanje moze da traje dok se prave
   shaderi, budi strpljiv.
4. Ako se igra pokrene, zapisi da li meniji imaju tekst i kako izgleda mapa.

### 5. Podesavanja u igri

- Kreni od rezolucije 1920x1080 u fullscreen ili borderless rezimu.
- Ako vidis bele pruge na neistrazenom delu mape ili obojene ivice na okeanu,
  promeni graficka podesavanja (kvalitet, senke, anti-aliasing) dok ne nestanu.
  Prijavljeno na M2 Air sa CrossOverom 22. Izvor: AppleGamingWiki.
- Za partije sa 4 ili vise igraca: Graphics > Quality na **Medium** i ogranici
  FPS na **30**, da ne bi preskakalo kadrove. Izvor: AppleGamingWiki.

### 6. Merenje

1. U bottle-u `aoe3` dodaj env promenljivu `MTL_HUD_ENABLED=1`
   (Bottle Settings > Environment ili preko `cxbottle.conf`) da vidis Metal HUD sa FPS-om.
   Napomena: HUD radi samo za Metal putanju, sa DXVK-om ide preko MoltenVK pa ga
   mozda neces videti. Tada koristi FPS brojac iz igre ili Steam overlay.
2. Odigraj istu skirmish mapu 5 minuta na 1080p i na native rezoluciji.
3. Upisi FPS u tabelu u [docs/notes/aoe3.md](notes/aoe3.md). Probaj i D3DMetal
   i DXMT radi poredjenja, i zapisi da li nestaje tekst.

## Poznati problemi

| Problem | Resenje | Izvor |
| --- | --- | --- |
| Nema teksta na dugmicima u meniju | prebaci grafiku na DXVK | AppleGamingWiki |
| Bele pruge i obojene ivice na mapi | promeni graficka podesavanja u igri | AppleGamingWiki |
| Seckanje u partijama 4+ igraca | Quality na Medium, FPS na 30 | AppleGamingWiki |
| Online multiplayer puca ili ne nalazi partije | nema pouzdanog resenja, testiraj i zapisi | izvestaji korisnika |
| Steam prozor crn ili prazan | restartuj Steam, ili u bottle-u probaj drugi graficki backend samo za Steam | AppleGamingWiki (GPTK strana) |

CodeWeavers ocena za AoE3 DE je mesovita i stara (Linux: "Installs, Will Not Run",
CrossOver 23.7.1). Za Mac nema svezih detaljnih prijava, pa je nasa faza 0 bas
korisna: posle testa vredi ostaviti prijavu na CodeWeavers stranici.

## Sta da upises u dnevnik

- verziju macOS-a, CrossOvera i backend (DXVK, D3DMetal, DXMT)
- da li se igra pokrece, da li meniji imaju tekst
- FPS na 1080p i native
- da li radi singleplayer, skirmish, LAN, online
- svaki korak koji je odstupio od ovog uputstva

## Izvori

- AppleGamingWiki, Age of Empires III: Definitive Edition:
  <https://applegamingwiki.com/wiki/Age_of_Empire_III:_Definitive_Edition>
- CodeWeavers kompatibilnost, AoE3 DE:
  <https://codeweavers.com/compatibility/crossover/age-of-empires-iii-definitive-edition>
- CodeWeavers forum, AoE3 DE:
  <https://codeweavers.com/compatibility/crossover/forum/age-of-empires-iii-definitive-edition>
- CrossOver changelog (D3DMetal 3.0 i DXMT u verziji 26):
  <https://www.codeweavers.com/crossover/changelog>
- CrossOver 26 najava:
  <https://www.codeweavers.com/blog/mjohnson/2026/02/10/crossover-26-cures-artificial-incompatibility-with-windows-games-on-mac>
