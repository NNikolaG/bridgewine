---
name: wine-patch-upstream
description: Pravila za pisanje i slanje patch-eva upstream u Wine (GitLab merge request) i DXMT (GitHub PR) - stil koda, commit poruke, testovi, Signed-off-by, clean-room. Koristi kad BridgeWine nadje bag u Wine-u ili DXMT-u i treba ga popraviti u njihovom kodu, ne zaobilaziti receptom. Use for any C/C++ change meant for Wine or DXMT upstream.
---

# wine-patch-upstream

Popravka u Wine/DXMT > zaobilaznica u receptu. Ali patch ide upstream, ne zivi zauvek u BridgeWine-u.

## Prvo
- Postoji li bag vec? Trazi: bugs.winehq.org, gitlab.winehq.org/wine/wine merge requests, github.com/3Shain/dxmt issues.
- Postoji = link u `docs/notes/<igra>.md`, komentar sa nasim podacima (chip, macOS, log). Ne dupliraj.
- Reprodukcija pre koda: minimalni korak + log linija koja dokazuje bag.

## Clean-room (obavezno)
- Nikad: disasembliran ili procuren Microsoft kod, kod iz GPTK/D3DMetal (Apple, zatvoren), CrossOver kod sa nejasnom licencom.
- Samo: javna dokumentacija (MSDN), ponasanje na pravom Windowsu, Wine testovi.
- Ne lepi kod iz drugih projekata bez provere licence (Wine = LGPL-2.1+).

## Wine
- Repo: `https://gitlab.winehq.org/wine/wine`. Fork na GitLabu, MR odatle. Mailing lista vise nije glavni put.
- Grana: od `master`. Rebase, ne merge.
- Commit naslov: `<komponenta>: <Opis velikim slovom sa tackom.>`
  - `d3d11: Implement ID3D11Device_CheckFeatureSupport for D3D11_FEATURE_...`
  - `ntdll: Return STATUS_... for ...`
  - komponenta = folder u `dlls/` ili `programs/`.
- Jedan logican korak = jedan commit. MR moze imati vise commitova, svaki mora da builduje.
- Ime i email pravi. `git commit -s` -> `Signed-off-by:`. Ime autora = korisnik (Nikola), ne Claude; pitaj korisnika za ime i email.
- Stil: prati okolni kod u tom fajlu. 4 razmaka, C, bez C++ komentara u starom kodu, `TRACE`/`FIXME`/`WARN` kao okolina.
- Testovi: `dlls/<x>/tests/`. Novo ponasanje = test koji prolazi na Windowsu (testbot) i na Wine-u posle patcha. `todo_wine` za ono sto jos ne radi.
- Build lokalno pre MR: `./configure && make -j` (ili samo izmenjeni dll), `make test` za taj dll.
- Bez macOS-only hakova u zajednickom kodu. Mac specificno: `dlls/winemac.drv` i `#ifdef __APPLE__` samo gde vec postoji obrazac.
- Reviewer trazi izmene = novi push na istu granu (force-push posle rebase-a OK u svom forku).

## DXMT
- Repo: `https://github.com/3Shain/dxmt`. C++, meson. PR na GitHubu.
- Prati njihov CONTRIBUTING/README ako postoji; inace stil okolnog koda.
- Build i test samo na Macu (Metal). Cloud thread ne moze da potvrdi.
- PR opis: igra, chip, macOS, pre/posle (log ili FPS), minimalni repro.

## U BridgeWine repou
- Patch dok ceka upstream: `patches/<projekat>/<NNNN>-<kratko>.patch` + red u beleskama sa linkom na MR/PR.
- Kad upstream primi: obrisi lokalni patch, recept prati verziju sa popravkom.
- Nikad ne commituj ceo Wine ili DXMT izvor u nas repo.

## Pre slanja (checklist)
1. Bag postoji upstream? Link.
2. Repro + log.
3. Build prolazi.
4. Test dodat ili razlog zasto ne moze.
5. Commit naslov u formatu, `Signed-off-by` korisnika.
6. Korisnik pregledao i odobrio. Slanje MR/PR je javno = uvek pitaj pre.

## Povezano
- Debug i logovi: skill `wine-gptk`.
- Staticka analiza pre slanja: Trail of Bits skills (ako instalirani).
