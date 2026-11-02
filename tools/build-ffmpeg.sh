#!/usr/bin/env bash
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
VERSION=8.0.3
BASE="$ROOT/.native"
PREFIX="$BASE/ffmpeg"
mkdir -p "$BASE/src" "$BASE/gnupg"
chmod 700 "$BASE/gnupg"
# Pinned zimg source, built locally; no runtime dependency on Homebrew.
ZIMG_VERSION=3.0.5
ZIMG_ARCHIVE="$BASE/src/zimg-$ZIMG_VERSION.tar.gz"
if [[ ! -f "$ZIMG_ARCHIVE" ]]; then curl --fail --location "https://github.com/sekrit-twc/zimg/archive/refs/tags/release-$ZIMG_VERSION.tar.gz" -o "$ZIMG_ARCHIVE"; fi
printf '%s  %s\n' a9a0226bf85e0d83c41a8ebe4e3e690e1348682f6a2a7838f1b8cbff1b799bcf "$ZIMG_ARCHIVE" | shasum -a 256 -c -
if [[ ! -d "$BASE/src/zimg-release-$ZIMG_VERSION" ]]; then tar -xzf "$ZIMG_ARCHIVE" -C "$BASE/src"; fi
(cd "$BASE/src/zimg-release-$ZIMG_VERSION" && ./autogen.sh && ./configure --prefix="$PREFIX" --enable-shared --disable-static && make -j4 && make install)
cp "$BASE/src/zimg-release-$ZIMG_VERSION/COPYING" "$PREFIX/zimg-LICENSE"
# FFmpeg's native AV1 decoder requires hardware support. Bundle dav1d for
# software decoding on Macs without an AV1 hardware decoder.
DAV1D_VERSION=1.5.3
DAV1D_ARCHIVE="$BASE/src/dav1d-$DAV1D_VERSION.tar.xz"
if [[ ! -f "$DAV1D_ARCHIVE" ]]; then curl --fail --location "https://downloads.videolan.org/pub/videolan/dav1d/$DAV1D_VERSION/dav1d-$DAV1D_VERSION.tar.xz" -o "$DAV1D_ARCHIVE"; fi
printf '%s  %s\n' 732010aa5ef461fa93355ed2c6c5fedb48ddc4b74e697eaabe8907eaeb943011 "$DAV1D_ARCHIVE" | shasum -a 256 -c -
DAV1D_SOURCE="$BASE/src/dav1d-$DAV1D_VERSION"
if [[ ! -d "$DAV1D_SOURCE" ]]; then tar -xf "$DAV1D_ARCHIVE" -C "$BASE/src"; fi
meson setup --reconfigure "$DAV1D_SOURCE/build" "$DAV1D_SOURCE" --prefix="$PREFIX" --libdir=lib --buildtype=release --default-library=shared --wrap-mode=nodownload -Denable_tools=false -Denable_tests=false
meson compile -C "$DAV1D_SOURCE/build" -j4
meson install -C "$DAV1D_SOURCE/build"
cp "$DAV1D_SOURCE/COPYING" "$PREFIX/dav1d-LICENSE"
export PKG_CONFIG_PATH="$PREFIX/lib/pkgconfig${PKG_CONFIG_PATH:+:$PKG_CONFIG_PATH}"
cd "$BASE/src"
if [[ ! -f "ffmpeg-$VERSION.tar.xz" ]]; then curl --fail --location "https://ffmpeg.org/releases/ffmpeg-$VERSION.tar.xz" -o "ffmpeg-$VERSION.tar.xz"; fi
curl --fail --location "https://ffmpeg.org/releases/ffmpeg-$VERSION.tar.xz.asc" -o "ffmpeg-$VERSION.tar.xz.asc"
curl --fail --location https://ffmpeg.org/ffmpeg-devel.asc -o ffmpeg-devel.asc
gpg --homedir "$BASE/gnupg" --import ffmpeg-devel.asc
gpg --homedir "$BASE/gnupg" --status-fd 1 --verify "ffmpeg-$VERSION.tar.xz.asc" "ffmpeg-$VERSION.tar.xz" > signature-status.txt
grep -q 'VALIDSIG FCF986EA15E6E293A5644F10B4322F04D67658D8' signature-status.txt
if [[ ! -d "ffmpeg-$VERSION" ]]; then tar -xf "ffmpeg-$VERSION.tar.xz"; fi
cd "ffmpeg-$VERSION"
./configure --prefix="$PREFIX" --enable-shared --disable-static --enable-libzimg --enable-libdav1d --disable-autodetect --disable-network --disable-doc --disable-debug --disable-everything --enable-avdevice --enable-indev=lavfi --enable-avcodec --enable-avformat --enable-avfilter --enable-swscale --enable-swresample --enable-ffmpeg --enable-ffprobe --enable-decoder=wrapped_avframe,h264,hevc,vp8,vp9,av1,libdav1d,mpeg4,mpeg2video,mjpeg,png,ffv1,rawvideo --enable-parser=h264,hevc,vp8,vp9,av1,mpeg4video,mpegvideo --enable-demuxer=mov,matroska,avi,mpegts,ogg,image2,concat --enable-protocol=file,pipe --enable-encoder=png,mjpeg,ffv1,mpeg4,rawvideo,wrapped_avframe --enable-muxer=image2,matroska,null,rawvideo --enable-filter=testsrc2,setpts,select,scale,format,color,buffer,buffersink,zscale,tonemap,geq,setparams
make -j4
make install
cp config.h "$PREFIX/config.h"
cp ffbuild/config.mak "$PREFIX/build-config.mak"
cp COPYING.LGPLv2.1 "$PREFIX/LICENSE"
shasum -a 256 "$BASE/src/ffmpeg-$VERSION.tar.xz" "$DAV1D_ARCHIVE" > "$PREFIX/source.sha256"
