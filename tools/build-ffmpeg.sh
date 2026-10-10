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
export PKG_CONFIG_PATH="$PREFIX/lib/pkgconfig${PKG_CONFIG_PATH:+:$PKG_CONFIG_PATH}"
cd "$BASE/src"
if [[ ! -f "ffmpeg-$VERSION.tar.xz" ]]; then curl --fail --location "https://ffmpeg.org/releases/ffmpeg-$VERSION.tar.xz" -o "ffmpeg-$VERSION.tar.xz"; fi
curl --fail --location "https://ffmpeg.org/releases/ffmpeg-$VERSION.tar.xz.asc" -o "ffmpeg-$VERSION.tar.xz.asc"
curl --fail --location https://ffmpeg.org/ffmpeg-devel.asc -o ffmpeg-devel.asc
gpg --homedir "$BASE/gnupg" --import ffmpeg-devel.asc
gpg --homedir "$BASE/gnupg" --status-fd 1 --verify "ffmpeg-$VERSION.tar.xz.asc" "ffmpeg-$VERSION.tar.xz" > signature-status.txt
rg -q 'VALIDSIG FCF986EA15E6E293A5644F10B4322F04D67658D8' signature-status.txt
if [[ ! -d "ffmpeg-$VERSION" ]]; then tar -xf "ffmpeg-$VERSION.tar.xz"; fi
cd "ffmpeg-$VERSION"
./configure --prefix="$PREFIX" --enable-shared --disable-static --enable-libzimg --disable-autodetect --disable-network --disable-doc --disable-debug --disable-everything --enable-avdevice --enable-indev=lavfi --enable-avcodec --enable-avformat --enable-avfilter --enable-swscale --enable-swresample --enable-ffmpeg --enable-ffprobe --enable-decoder=wrapped_avframe,h264,hevc,vp8,vp9,av1,mpeg4,mpeg2video,mjpeg,png,ffv1,rawvideo --enable-parser=h264,hevc,vp8,vp9,av1,mpeg4video,mpegvideo --enable-demuxer=mov,matroska,avi,mpegts,ogg,image2,concat --enable-protocol=file,pipe --enable-encoder=png,mjpeg,ffv1,mpeg4,rawvideo,wrapped_avframe --enable-muxer=image2,matroska,null,rawvideo --enable-filter=testsrc2,setpts,select,scale,format,color,buffer,buffersink,zscale,tonemap,geq,setparams
make -j4
make install
cp config.h "$PREFIX/config.h"
cp ffbuild/config.mak "$PREFIX/build-config.mak"
cp COPYING.LGPLv2.1 "$PREFIX/LICENSE"
shasum -a 256 "$BASE/src/ffmpeg-$VERSION.tar.xz" > "$PREFIX/source.sha256"
