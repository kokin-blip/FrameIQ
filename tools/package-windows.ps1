param([string]$BuildRoot='C:\frameiq-native')
$ErrorActionPreference='Stop'
$repo=Split-Path $PSScriptRoot -Parent
$native="$repo/.native/ffmpeg"
$env:FFMPEG_DIR=$native; $env:PATH="$native/bin;$env:PATH"
$env:LIBCLANG_PATH='C:\Program Files\LLVM\bin'
cargo build --locked --release -p frameiq-worker -p frameiq-ai-worker
if($LASTEXITCODE){throw 'Worker release build failed'}
$stage="$repo/apps/desktop/src-tauri"
if(Test-Path "$stage/native"){Remove-Item "$stage/native" -Recurse -Force}
New-Item -ItemType Directory -Force "$stage/binaries","$stage/native" | Out-Null
Copy-Item "$repo/target/release/frameiq-worker.exe" "$stage/binaries/frameiq-worker-x86_64-pc-windows-msvc.exe" -Force
Copy-Item "$repo/target/release/frameiq-ai-worker.exe" "$stage/binaries/frameiq-ai-worker-x86_64-pc-windows-msvc.exe" -Force
# All native dependencies are collected from the controlled install, then audited recursively.
Get-ChildItem "$native/bin" -Filter '*.dll' | Copy-Item -Destination "$stage/native" -Force
Copy-Item "$native/LICENSE" "$stage/native/FFmpeg-LICENSE" -Force
Copy-Item "$native/zimg-LICENSE" "$stage/native/zimg-LICENSE" -Force
Copy-Item "$native/build-config.mak" "$stage/native/build-config.mak" -Force
# The ORT archive must have been verified against the committed runtime lock.
$ort="$repo/.native/onnxruntime"
if(!(Test-Path "$ort/lib/onnxruntime.dll")){throw 'Run tools/prepare-onnxruntime.mjs first.'}
Get-ChildItem "$ort/lib" -Filter '*.dll' | Copy-Item -Destination "$stage/native" -Force
$crt=Get-ChildItem "$env:VCToolsRedistDir/x64" -Directory -Filter 'Microsoft.VC*.CRT' | Select-Object -First 1
if(!$crt){throw 'MSVC runtime redistributable directory is missing.'}
Get-ChildItem $crt.FullName -Filter '*.dll' | Copy-Item -Destination "$stage/native" -Force
Copy-Item "$ort/LICENSE" "$stage/native/ONNXRuntime-LICENSE" -Force
Copy-Item "$repo/docs/licenses/OpenCV-Apache-2.0.txt","$repo/docs/licenses/FaceDecoder-NOTICE.txt" "$stage/native" -Force
New-Item -ItemType Directory -Force "$repo/.cache/windows-evidence" | Out-Null
node "$repo/tools/audit-windows-dlls.mjs" "$stage" "$repo/.cache/windows-evidence/dependencies.json"
if($LASTEXITCODE){throw 'Native DLL dependency audit failed'}
Push-Location "$repo/apps/desktop"
npx tauri build --bundles nsis --config '{"bundle":{"resources":{"native/*":"."}}}'
Pop-Location
if($LASTEXITCODE){throw 'NSIS packaging failed'}
Write-Host 'Installer built; install and run offline on a clean Windows 11 x64 machine before release.'
