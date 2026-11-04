#!/bin/sh
# Rosetta+Wine vs arm64 Wine+FEX: cijena #UD izuzetka iz AoE3 zastite.
# Pokreni iz korijena repoa na Apple Silicon Macu:  sh bench/fex-exceptions/run.sh [count]
set -u
N=${1:-100000}
HERE=$(cd "$(dirname "$0")" && pwd)
BW_HOME=${BW_HOME:-$HOME/Games/BridgeWine}
EXE="$HERE/opbench.exe"
export WINEDEBUG=-all

echo "== 1. native arm64 macOS SIGILL (pod)"
clang -arch arm64 -O2 -o "$HERE/sigill_arm64" "$HERE/sigill_arm64.c" && "$HERE/sigill_arm64" "$N"

echo "== 2. Rosetta + WineHQ (danas)"
X64_WINE=$(ls -d "$BW_HOME"/runtimes/wine-devel-*.app 2>/dev/null | tail -1)/Contents/Resources/wine/bin/wine
if [ -x "$X64_WINE" ]; then
  # odvojen prefiks da ne diramo aoe3-de bottle
  WINEPREFIX="$BW_HOME/bottles/x64-bench" "$X64_WINE" wineboot -i >/dev/null 2>&1
  WINEPREFIX="$BW_HOME/bottles/x64-bench" "$X64_WINE" "$EXE" "$N"
else
  echo "nema WineHQ runtime-a (bw runtime install), preskacem"
fi

echo "== 3. arm64 Wine + FEX (bez Rosette)"
cargo run -q -- runtime install-arm64 || exit 1
WINEPREFIX="$BW_HOME/bottles/arm64-fex" WINEARM64ECEMU=1 \
  "$BW_HOME/runtimes/wine-arm64-fex-0.1.0/bin/wine" "$EXE" "$N"
echo "rc=$? (0 = svi izuzeci na pravom mjestu)"
