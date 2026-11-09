#!/bin/sh
# Lokalni build naseg arm64 Wine-a (isto sto radi .github/workflows/wine-arm64.yml).
# Izvor i alati su vec u $BW_HOME/src/arm64 (wine sa patch-evima, llvm-mingw).
#   sh scripts/build-arm64-wine.sh
set -eu
BW_HOME=${BW_HOME:-$HOME/Games/BridgeWine}
D=$BW_HOME/src/arm64
DEST=$BW_HOME/runtimes/wine-arm64-bw
mkdir -p "$D/deps/lib" "$D/build"
lipo -thin arm64 "$BW_HOME"/runtimes/wine-devel-*.app/Contents/Resources/wine/lib/libMoltenVK.dylib \
  -output "$D/deps/lib/libMoltenVK.dylib"
export PATH="/opt/homebrew/opt/bison/bin:/opt/homebrew/opt/flex/bin:$D/llvm-mingw-20260922-ucrt-macos-universal/bin:$PATH"
cd "$D/build"
[ -f Makefile ] || ../wine/configure --prefix="$DEST" --enable-archs=aarch64,arm64ec ac_cv_func_pipe2=no \
  LDFLAGS="-L$D/deps/lib -L/opt/homebrew/lib" CPPFLAGS="-I/opt/homebrew/include" \
  --without-x --without-gstreamer --without-sdl --without-cups --without-sane \
  --without-gphoto --without-krb5 --without-pcap --without-usb --without-v4l2 \
  --without-capi --without-pulse --without-alsa --without-oss --without-dbus \
  --without-udev --without-ffmpeg --disable-tests >configure.out 2>&1 || { tail -30 configure.out; exit 1; }
grep -E 'SONAME_LIB(VULKAN|FREETYPE|GNUTLS)' include/config.h
make -j"$(sysctl -n hw.ncpu)" >make.log 2>&1 || { tail -60 make.log; exit 1; }
make install >install.log 2>&1
cp "$D/deps/lib/libMoltenVK.dylib" "$DEST/lib/"
echo "BW_ARM64_WINE_OK $DEST"
