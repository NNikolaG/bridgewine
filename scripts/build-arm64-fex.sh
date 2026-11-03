#!/usr/bin/env bash
# Eksperimentalni runtime bez Rosette: native arm64 Wine (ARM64EC) + FEX-Emu
# kao x86_64 -> arm64 prevodilac unutar procesa (kao Windows on ARM).
#
# Radi samo na macOS arm64 (Apple Silicon). Rezultat je samostalan Wine u
# $OUT (podrazumevano ~/Games/BridgeWine/runtimes/wine-arm64-fex) sa FEX DLL-om.
#
# Izvori (sve LGPL/MIT, pinovano):
#   Wine:       citi94/wine-macos-arm64 (Wine 11.10 + macOS arm64 port)
#   Patchevi:   Irz-Z/wine-macos-native v0.1.0 (TEB u TSD slotu, ARM64EC emulator)
#               kopirani u patches/arm64-fex/
#   FEX:        FEX-Emu/FEX tag FEX-2609
#   Toolchain:  llvm-mingw 20260922 (+ mali patch za NtCurrentTeb)
#
# Upotreba:
#   scripts/build-arm64-fex.sh            # sve
#   JOBS=8 OUT=/neki/dir scripts/build-arm64-fex.sh
#   STEP=fex scripts/build-arm64-fex.sh   # samo jedan korak (toolchain|wine|fex|install)
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
PATCHES="$ROOT/patches/arm64-fex"
WORK="${WORK:-$HOME/Games/BridgeWine/build/arm64-fex}"
OUT="${OUT:-$HOME/Games/BridgeWine/runtimes/wine-arm64-fex}"
JOBS="${JOBS:-$(sysctl -n hw.ncpu 2>/dev/null || echo 8)}"
STEP="${STEP:-all}"

WINE_REPO="https://github.com/citi94/wine-macos-arm64"
WINE_COMMIT="0cc8848b607572cfeacf3e550ef696bd63a43955"
FEX_REPO="https://github.com/FEX-Emu/FEX"
FEX_TAG="FEX-2609"
FEX_COMMIT="395b132f346b1a45def246d10c52245edba1ef02"
LLVM_MINGW="llvm-mingw-20260922-ucrt-macos-universal"
LLVM_MINGW_URL="https://github.com/mstorsjo/llvm-mingw/releases/download/20260922/$LLVM_MINGW.tar.xz"
FEX_SUBMODULES=(External/fmt External/range-v3 External/rpmalloc External/unordered_dense
                External/xxhash External/vixl External/zydis Source/Common/cpp-optparse)

log() { printf '\n==> %s\n' "$*"; }
die() { printf 'GRESKA: %s\n' "$*" >&2; exit 1; }

[ "$(uname -s)" = Darwin ] || die "treba macOS"
[ "$(uname -m)" = arm64 ] || die "treba arm64 Mac"
[ "$(sysctl -n sysctl.proc_translated 2>/dev/null || echo 0)" = 0 ] || die "shell radi pod Rosettom; pokreni nativni arm64 terminal"
for t in git cmake make bison flex curl codesign; do
  command -v "$t" >/dev/null || die "fali $t (brew install cmake bison flex)"
done
# Homebrew bison/flex su noviji od sistemskih; Wine ih trazi.
for p in /opt/homebrew/opt/bison/bin /opt/homebrew/opt/flex/bin; do
  [ -d "$p" ] && PATH="$p:$PATH"
done

mkdir -p "$WORK"
TC="$WORK/$LLVM_MINGW"

step_toolchain() {
  if [ -x "$TC/bin/arm64ec-w64-mingw32-clang" ] && [ -f "$TC/.bw-patched" ]; then
    log "llvm-mingw vec postoji"; return
  fi
  log "llvm-mingw 20260922"
  rm -rf "$TC"
  curl -fL --retry 3 -o "$WORK/$LLVM_MINGW.tar.xz" "$LLVM_MINGW_URL"
  tar -C "$WORK" -xf "$WORK/$LLVM_MINGW.tar.xz"
  xattr -dr com.apple.quarantine "$TC" 2>/dev/null || true
  (cd "$TC" && patch -p1 < "$PATCHES/llvm-mingw/0001-ntcurrentteb-tsd.patch")
  touch "$TC/.bw-patched"
}

step_wine() {
  local src="$WORK/wine-src" build="$WORK/wine-build"
  if [ ! -d "$src/.git" ]; then
    log "Wine izvor ($WINE_COMMIT)"
    git init -q "$src"
    git -C "$src" remote add origin "$WINE_REPO"
  fi
  if [ "$(git -C "$src" rev-parse HEAD 2>/dev/null)" != "$WINE_COMMIT" ] || [ ! -f "$src/.bw-patched" ]; then
    git -C "$src" fetch -q --depth 1 origin "$WINE_COMMIT"
    git -C "$src" checkout -q -f FETCH_HEAD
    git -C "$src" clean -qfdx
    log "Wine patchevi"
    (cd "$src" && patch -p1 < "$PATCHES/wine/0001-macos-native-arm64-tsd-teb-arm64ec.patch")
    touch "$src/.bw-patched"
  fi
  mkdir -p "$build"
  if [ ! -f "$build/Makefile" ]; then
  log "Wine configure (arm64ec + aarch64)"
  # Unix strana: Appleov clang (/usr/bin), ne llvm-mingw-ov "clang" koji je prvi u PATH-u.
  (cd "$build" && PATH="$TC/bin:$PATH" "$src/configure" -q \
      --prefix="$OUT" \
      --enable-archs=arm64ec,aarch64 \
      --without-x --disable-tests \
      CC=/usr/bin/clang CXX=/usr/bin/clang++ ac_cv_func_pipe2=no) \
    || { tail -60 "$build/config.log"; die "Wine configure pao"; }
  fi
  log "Wine build (-j$JOBS), traje 20-60 min"
  PATH="$TC/bin:$PATH" make -C "$build" -j"$JOBS" >"$WORK/wine-build.log" 2>&1 \
    || { tail -40 "$WORK/wine-build.log"
         log "Dijagnostika: ponavljam pad sa winebuild -v"
         command -v aarch64-w64-mingw32-clang arm64ec-w64-mingw32-clang llvm-dlltool lld-link || true
         PATH="$TC/bin:$PATH" command -v aarch64-w64-mingw32-clang arm64ec-w64-mingw32-clang llvm-dlltool lld-link || true
         PATH="$TC/bin:$PATH" aarch64-w64-mingw32-clang -print-prog-name=llvm-dlltool || true
         PATH="$TC/bin:$PATH" make -C "$build" -j1 WINEBUILD="$build/tools/winebuild/winebuild -v" 2>&1 | tail -40
         die "Wine build pao (vidi $WORK/wine-build.log)"; }
}

step_fex() {
  local src="$WORK/fex-src" build="$WORK/fex-build"
  if [ ! -f "$src/.bw-patched" ]; then
    log "FEX izvor ($FEX_TAG)"
    rm -rf "$src"
    git clone -q --depth 1 -b "$FEX_TAG" "$FEX_REPO" "$src"
    [ "$(git -C "$src" rev-parse HEAD)" = "$FEX_COMMIT" ] || die "FEX tag se promenio"
    git -C "$src" submodule update -q --init --depth 1 "${FEX_SUBMODULES[@]}"
    git -C "$src" apply "$PATCHES/fex/0001-load-teb-from-tsd-slot-and-wow64-guest-base.patch"
    git -C "$src/External/vixl" apply "$PATCHES/fex/0002-vixl-include-map.patch"
    touch "$src/.bw-patched"
  fi
  log "FEX ARM64EC DLL"
  PATH="$TC/bin:$PATH" cmake -S "$src" -B "$build" -G "Unix Makefiles" \
    -DCMAKE_BUILD_TYPE=Release -DCMAKE_POLICY_VERSION_MINIMUM=3.5 \
    -DCMAKE_TOOLCHAIN_FILE="$src/Data/CMake/toolchain_mingw.cmake" \
    -DMINGW_TRIPLE=arm64ec-w64-mingw32 \
    -DENABLE_LTO=False -DBUILD_TESTING=False -DENABLE_JEMALLOC_GLIBC_ALLOC=False \
    -DTUNE_CPU=none -DENABLE_CCACHE=False \
    -DCMAKE_SHARED_LINKER_FLAGS=-Wl,--section-alignment=16384 \
    -DCMAKE_C_FLAGS=-DWINE_MACOS_TSD_TEB -DCMAKE_CXX_FLAGS=-DWINE_MACOS_TSD_TEB >/dev/null
  PATH="$TC/bin:$PATH" cmake --build "$build" --target arm64ecfex --parallel "$JOBS" >"$WORK/fex-build.log" 2>&1 \
    || { tail -40 "$WORK/fex-build.log"; die "FEX build pao (vidi $WORK/fex-build.log)"; }
  ls -la "$build/Bin/libarm64ecfex.dll"
}

step_install() {
  log "Instalacija u $OUT"
  rm -rf "$OUT"
  make -C "$WORK/wine-build" install >"$WORK/wine-install.log" 2>&1 \
    || { tail -20 "$WORK/wine-install.log"; die "make install pao"; }
  # ntdll (arm64ec) podrazumevano ucitava system32\xtajit64.dll; ako ga nema u
  # prefiksu, Wine ga nalazi kao builtin u aarch64-windows. Stavljamo oba imena.
  cp "$WORK/fex-build/Bin/libarm64ecfex.dll" "$OUT/lib/wine/aarch64-windows/libarm64ecfex.dll"
  cp "$WORK/fex-build/Bin/libarm64ecfex.dll" "$OUT/lib/wine/aarch64-windows/xtajit64.dll"
  # JIT (Wine + FEX) na macOS arm64: potpisi loader sa JIT entitlementima.
  local ent="$WORK/jit.entitlements"
  cat >"$ent" <<'EOF'
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
  <key>com.apple.security.cs.allow-jit</key><true/>
  <key>com.apple.security.cs.allow-unsigned-executable-memory</key><true/>
  <key>com.apple.security.cs.disable-library-validation</key><true/>
</dict></plist>
EOF
  for b in "$OUT/bin/wine" "$OUT/bin/wineserver"; do
    [ -f "$b" ] && codesign -f -s - --entitlements "$ent" "$b"
  done
  cp "$PATCHES/fex/LICENSE-FEX-MIT" "$OUT/LICENSE-FEX"
  cat >"$OUT/BW-RUNTIME.txt" <<EOF
BridgeWine eksperimentalni runtime: arm64 Wine + FEX (bez Rosette)
wine: $WINE_REPO @ $WINE_COMMIT + patches/arm64-fex/wine
fex:  $FEX_REPO $FEX_TAG @ $FEX_COMMIT + patches/arm64-fex/fex
toolchain: $LLVM_MINGW
Pokretanje x86_64 programa: WINEARM64ECEMU=1, a u bottle-u
  HKLM\\Software\\Microsoft\\Wow64\\amd64 (Default) = libarm64ecfex.dll
(scripts/bench-sigill.sh to radi sam)
EOF
  log "Gotovo: $OUT/bin/wine"
}

case "$STEP" in
  all) step_toolchain; step_wine; step_fex; step_install ;;
  toolchain) step_toolchain ;;
  wine) step_toolchain; step_wine ;;
  fex) step_toolchain; step_fex ;;
  install) step_install ;;
  *) die "nepoznat STEP=$STEP" ;;
esac
