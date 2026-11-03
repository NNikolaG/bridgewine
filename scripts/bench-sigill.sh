#!/usr/bin/env bash
# Pokrece tools/sigill-bench pod zadatim Wine-om u privremenom prefiksu.
#   scripts/bench-sigill.sh <putanja/do/wine> <sigill_bench.exe> [rosetta|fex] [iteracija]
# Izlaz: RESULT linije (cena jednog AoE3-stila izuzetka u mikrosekundama).
set -euo pipefail
WINE="$1"; EXE="$2"; MODE="${3:-rosetta}"; ITERS="${4:-200000}"
PFX="$(mktemp -d "${TMPDIR:-/tmp}/bw-bench-XXXXXX")"
export WINEPREFIX="$PFX" WINEDEBUG="${WINEDEBUG:--all}" WINEDLLOVERRIDES="mscoree,mshtml=;winedbg.exe=d"
if [ "$MODE" = fex ]; then
  export WINEARM64ECEMU=1
fi
echo "== $MODE: $WINE"
"$WINE" wineboot -i >/dev/null 2>&1 || true
if [ "$MODE" = fex ]; then
  "$WINE" reg add 'HKLM\Software\Microsoft\Wow64\amd64' /ve /t REG_SZ /d xtajit64.dll /f >/dev/null 2>&1 || true
fi
[ -d "$PFX/drive_c" ] || { echo "ERROR wineboot nije napravio prefiks"; "$WINE" wineboot -i 2>&1 | tail -30; exit 1; }
cp "$EXE" "$PFX/drive_c/sigill_bench.exe"
status=0
"$WINE" 'C:\sigill_bench.exe' "$ITERS" || status=$?
"$(dirname "$WINE")/wineserver" -k 2>/dev/null || true
rm -rf "$PFX"
exit $status
