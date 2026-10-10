param([string]$BuildRoot = 'C:\frameiq-native')
$ErrorActionPreference = 'Stop'
$repo = Split-Path $PSScriptRoot -Parent
$commit = 'f86152d27fbc290a4152ebd1cecc0bd9edaa9859'
if ($BuildRoot.Contains(' ')) { throw 'Choose a native build path without spaces.' }
New-Item -ItemType Directory -Force $BuildRoot | Out-Null
$vc = Join-Path $BuildRoot 'vcpkg'
if (!(Test-Path "$vc/.git")) { git clone --filter=blob:none https://github.com/microsoft/vcpkg.git $vc; if($LASTEXITCODE){throw 'vcpkg clone failed'} }
git -C $vc checkout --detach $commit
if($LASTEXITCODE){throw 'Pinned vcpkg checkout failed'}
& "$vc/bootstrap-vcpkg.bat" -disableMetrics
if($LASTEXITCODE){throw 'vcpkg bootstrap failed'}
$overlay = Join-Path $BuildRoot 'overlay/ffmpeg'
New-Item -ItemType Directory -Force $overlay | Out-Null
Copy-Item "$vc/ports/ffmpeg/*" $overlay -Recurse -Force
# Retain the reviewed MSVC patches, fetching the pinned official 8.0.3 source archive.
$port = Get-Content "$overlay/portfile.cmake" -Raw
$header = 'vcpkg_download_distfile(ARCHIVE URLS "https://ffmpeg.org/releases/ffmpeg-8.0.3.tar.xz" FILENAME "ffmpeg-8.0.3.tar.xz" SHA512 e024e6a8b93cf8765f7d3ce6ec5b3e0b85d2f939cea42200d681e6cdbd5cfbd2cff237c23ce882c98ab0ce6a52b04b17ae5f90e368cf5360caa919fbcb8a4305)' + "`n" + 'vcpkg_extract_source_archive(SOURCE_PATH ARCHIVE "${ARCHIVE}"'
$port = $port.Replace('vcpkg_from_github(', $header)
$port = $port -replace '(?m)^\s*(OUT_SOURCE_PATH|REPO|REF|SHA512|HEAD_REF) .+\r?\n',''
$restricted = '--disable-gpl --disable-nonfree --disable-network --disable-everything --enable-avdevice --enable-indev=lavfi --enable-avcodec --enable-avformat --enable-avfilter --enable-swscale --enable-swresample --enable-ffmpeg --enable-ffprobe --enable-libzimg --enable-decoder=wrapped_avframe,h264,hevc,vp8,vp9,av1,mpeg4,mpeg2video,mjpeg,png,ffv1,rawvideo --enable-parser=h264,hevc,vp8,vp9,av1,mpeg4video,mpegvideo --enable-demuxer=mov,matroska,avi,mpegts,ogg,image2,concat --enable-protocol=file,pipe --enable-encoder=png,mjpeg,ffv1,mpeg4,rawvideo,wrapped_avframe --enable-muxer=image2,matroska,null,rawvideo --enable-filter=testsrc2,setpts,select,scale,format,color,buffer,buffersink,zscale,tonemap,geq,setparams'
$port = $port.Replace('set(OPTIONS_RELEASE "--enable-optimizations")', 'string(APPEND OPTIONS " ' + $restricted + '")' + "`n" + 'set(OPTIONS_RELEASE "--enable-optimizations")')
Set-Content "$overlay/portfile.cmake" $port
$manifest = Get-Content "$overlay/vcpkg.json" -Raw | ConvertFrom-Json
$manifest.version = '8.0.3'; $manifest | Add-Member -NotePropertyName 'port-version' -NotePropertyValue 0 -Force
$manifest.dependencies += 'zimg'
$manifest | ConvertTo-Json -Depth 50 | Set-Content "$overlay/vcpkg.json"
$triplets = Join-Path $BuildRoot 'triplets'
New-Item -ItemType Directory -Force $triplets | Out-Null
@'
set(VCPKG_TARGET_ARCHITECTURE x64)
set(VCPKG_CRT_LINKAGE dynamic)
set(VCPKG_LIBRARY_LINKAGE dynamic)
set(VCPKG_BUILD_TYPE release)
'@ | Set-Content "$triplets/x64-frameiq.cmake"
$overlayRoot = Split-Path $overlay -Parent
Copy-Item "$repo/tools/vcpkg-overlay/zimg" $overlayRoot -Recurse -Force
& "$vc/vcpkg.exe" install 'ffmpeg[avcodec,avdevice,avfilter,avformat,swresample,swscale,ffmpeg,ffprobe]:x64-frameiq' "--overlay-ports=$overlayRoot" "--overlay-triplets=$triplets" --disable-metrics
if($LASTEXITCODE){throw 'Restricted FFmpeg native build failed'}
$prefix = "$vc/installed/x64-frameiq"
$native = "$repo/.native/ffmpeg"
New-Item -ItemType Directory -Force $native | Out-Null
Copy-Item "$prefix/*" $native -Recurse -Force
Copy-Item "$prefix/share/ffmpeg/copyright" "$native/LICENSE" -Force
Copy-Item "$prefix/share/zimg/copyright" "$native/zimg-LICENSE" -Force
$ffmpeg = Get-ChildItem $prefix -Recurse -Filter ffmpeg.exe | Select-Object -First 1
$probe = Get-ChildItem $prefix -Recurse -Filter ffprobe.exe | Select-Object -First 1
Copy-Item $ffmpeg.FullName "$native/bin/ffmpeg.exe" -Force
Copy-Item $probe.FullName "$native/bin/ffprobe.exe" -Force
$env:PATH = "$native/bin;$env:PATH"
& "$native/bin/ffmpeg.exe" -buildconf 2>&1 | Set-Content "$native/build-config.mak"
$config = Get-Content "$native/build-config.mak" -Raw
if($config -match '--enable-(gpl|nonfree)' -or $config -notmatch '--enable-libzimg') { throw 'Native license/color configuration failed validation' }
$env:FFMPEG_DIR = $native
$env:VCPKG_ROOT = $vc
$env:VCPKGRS_TRIPLET = 'x64-frameiq'
$env:VCPKGRS_DYNAMIC = '1'
$env:LIBCLANG_PATH = 'C:\Program Files\LLVM\bin'
& cargo build --locked -p frameiq-worker -p frameiq-ai-worker
if($LASTEXITCODE){throw 'Worker build failed'}
& cargo test --locked --workspace --exclude frameiq-desktop --features frameiq-application/fault-injection
if($LASTEXITCODE){throw 'Windows fidelity tests failed'}
Write-Host 'Native files built. Next run tools/package-windows.ps1; clean-machine verification remains required.'
