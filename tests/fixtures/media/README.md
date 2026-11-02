# Synthetic AV1 regression fixture

`av1-fragmented.mp4` contains ten 160×120 test-pattern frames at 10 fps in a fragmented MP4, matching the AV1-in-MP4 container used by YouTube downloads. It contains no user media or audio. SHA-256: `d06660fbfb3ec909bd640221641d4b9462a77e7bac33f63e6ee37477c888b376`.

Generated with FFmpeg 7.1 and libaom-av1 using the development-only imageio-ffmpeg 0.6.0 macOS arm64 binary:

```sh
ffmpeg -f lavfi -i 'testsrc2=size=160x120:rate=10:duration=1' \
  -c:v libaom-av1 -cpu-used 8 -crf 30 -g 5 -threads 1 -pix_fmt yuv420p \
  -color_range tv -colorspace bt709 -color_trc bt709 -color_primaries bt709 \
  -movflags +dash+frag_keyframe+empty_moov av1-fragmented.mp4
```

The fixture is committed because the restricted shipping FFmpeg build decodes AV1 but does not include an AV1 encoder. Tests require no additional downloads or encoders.
