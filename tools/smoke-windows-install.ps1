$ErrorActionPreference='Stop'
$repo=Split-Path $PSScriptRoot -Parent
$evidence="$repo/.cache/windows-evidence"
New-Item -ItemType Directory -Force $evidence | Out-Null
Start-Transcript -Path "$evidence/installed-smoke.log" -Force
try {
  $installer=Get-ChildItem "$repo/target/release/bundle/nsis" -Filter '*.exe' | Select-Object -First 1
  if(!$installer){throw 'Private installer is missing'}
  $install=Join-Path $env:RUNNER_TEMP 'frameiq-installed'
  if(Test-Path $install){Remove-Item $install -Recurse -Force}
  $process=Start-Process -FilePath $installer.FullName -ArgumentList @('/S',"/D=$install") -Wait -PassThru
  if($process.ExitCode){throw "Installer exited $($process.ExitCode)"}
  $worker=Get-ChildItem $install -Recurse -Filter 'frameiq-worker.exe' | Select-Object -First 1
  if(!$worker){throw 'Installed media worker is missing'}
  $fixtures=Join-Path $env:RUNNER_TEMP 'frameiq-installed-fixtures'
  New-Item -ItemType Directory -Force $fixtures | Out-Null
  $ffmpeg="$repo/.native/ffmpeg/bin/ffmpeg.exe"
  foreach($source in @('sdr','pq','hlg')) {
    $transfer=if($source -eq 'pq'){'smpte2084'}elseif($source -eq 'hlg'){'arib-std-b67'}else{'bt709'}
    $primaries=if($source -eq 'sdr'){'bt709'}else{'bt2020'}
    $matrix=if($source -eq 'sdr'){'bt709'}else{'bt2020nc'}
    $format=if($source -eq 'sdr'){'yuv420p'}else{'yuv420p10le'}
    & $ffmpeg -hide_banner -loglevel error -y -f lavfi -i 'testsrc2=size=160x120:rate=4:duration=1' -vf "format=$format,setparams=color_primaries=${primaries}:color_trc=${transfer}:colorspace=$matrix" -c:v ffv1 -level 3 -pix_fmt $format -color_primaries $primaries -color_trc $transfer -colorspace $matrix -color_range tv "$fixtures/$source.mkv"
    if($LASTEXITCODE){throw "Fixture generation failed: $source"}
  }
  $manifest=Get-Content "$repo/models/manifests/yunet-2023mar.json" -Raw | ConvertFrom-Json
  $model="$fixtures/yunet.onnx"
  Invoke-WebRequest -Uri $manifest.sourceUrl -OutFile $model
  if((Get-FileHash $model -Algorithm SHA256).Hash.ToLower() -ne $manifest.sha256){throw 'Reviewed model checksum failed'}
  @('Win32_OperatingSystem','Win32_Processor','Win32_VideoController') | ForEach-Object { Get-CimInstance -ClassName $_ } | Select-Object Caption,Version,TotalVisibleMemorySize,Name,NumberOfLogicalProcessors,DriverVersion | ConvertTo-Json -Depth 4 | Set-Content "$evidence/hardware.json"
  node "$repo/tools/smoke-installed-workers.mjs" $worker.DirectoryName $fixtures "$evidence/installed-workers.json" $model
  if($LASTEXITCODE){throw 'Installed worker smoke failed'}
} finally { Stop-Transcript }
