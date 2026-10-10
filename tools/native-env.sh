#!/usr/bin/env bash
# Source this file from the repository root before cargo commands.
export PKG_CONFIG_PATH="$PWD/.native/ffmpeg/lib/pkgconfig${PKG_CONFIG_PATH:+:$PKG_CONFIG_PATH}"
export DYLD_LIBRARY_PATH="$PWD/.native/ffmpeg/lib${DYLD_LIBRARY_PATH:+:$DYLD_LIBRARY_PATH}"
export LD_LIBRARY_PATH="$PWD/.native/ffmpeg/lib${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"
export PATH="$PWD/.native/ffmpeg/bin:$PATH"
