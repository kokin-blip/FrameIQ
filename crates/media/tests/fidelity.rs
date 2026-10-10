use frameiq_contracts::*;
use frameiq_media::{decode, index};
use std::{
    path::{Path, PathBuf},
    process::Command,
};
fn ffmpeg() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join(if cfg!(windows) {
        "../../.native/ffmpeg/bin/ffmpeg.exe"
    } else {
        "../../.native/ffmpeg/bin/ffmpeg"
    })
}
fn run(args: &[&str]) {
    let out = Command::new(ffmpeg())
        .args(["-hide_banner", "-loglevel", "error"])
        .args(args)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
}
fn fixture(dir: &Path, vfr: bool, bframes: bool) -> PathBuf {
    let path = dir.join("source.mkv");
    let mut args = vec![
        "-f",
        "lavfi",
        "-i",
        "testsrc2=size=160x120:rate=10:duration=1",
    ];
    if vfr {
        args.extend(["-vf", "setpts=N*N/(10*TB)", "-fps_mode", "passthrough"])
    }
    args.extend(["-c:v", if bframes { "mpeg4" } else { "ffv1" }]);
    if bframes {
        args.extend(["-bf", "2", "-g", "5"])
    }
    args.push(path.to_str().unwrap());
    run(&args);
    path
}
#[test]
fn exact_export_matches_sequential_reference_for_cfr_vfr_and_b_frames() {
    for (vfr, bframes) in [(false, false), (true, false), (false, true)] {
        let d = tempfile::tempdir().unwrap();
        let source = fixture(d.path(), vfr, bframes);
        let mut frames = vec![];
        index(source.to_str().unwrap(), None, |m| {
            if let WorkerMessage::Frames { frames: batch } = m {
                frames.extend(batch)
            }
            Ok(())
        })
        .unwrap();
        assert_eq!(frames.len(), 10);
        if vfr {
            assert!(frames[9].time_seconds > 7.0)
        }
        let raw = d.path().join("reference.rgb");
        run(&[
            "-i",
            source.to_str().unwrap(),
            "-vf",
            "scale=in_range=tv:out_range=pc:in_color_matrix=bt601:flags=bicubic",
            "-pix_fmt",
            "rgb24",
            "-f",
            "rawvideo",
            "-fps_mode",
            "passthrough",
            raw.to_str().unwrap(),
        ]);
        let reference = std::fs::read(raw).unwrap();
        assert_eq!(reference.len(), 10 * 160 * 120 * 3);
        for f in &frames {
            let ordinal = f.ordinal.parse::<u64>().unwrap();
            let output = d.path().join(format!("{ordinal}.png"));
            let anchor = frames
                .iter()
                .rev()
                .find(|a| {
                    a.ordinal.parse::<u64>().unwrap() <= ordinal
                        && a.key_frame
                        && a.pts_ticks.is_some()
                })
                .map(|a| SeekAnchor {
                    ordinal: a.ordinal.parse().unwrap(),
                    pts: a.pts_ticks.as_ref().unwrap().parse().unwrap(),
                    time_seconds: a.time_seconds,
                });
            decode(
                source.to_str().unwrap(),
                0,
                ordinal,
                f.pts_ticks.as_ref().map(|p| p.parse().unwrap()),
                anchor.as_ref(),
                &output,
                None,
                &ExportOptions::default(),
            )
            .unwrap();
            let pixels = image::open(output).unwrap().to_rgb8().into_raw();
            let start = ordinal as usize * 160 * 120 * 3;
            assert_eq!(
                pixels,
                &reference[start..start + 160 * 120 * 3],
                "Frame {ordinal}; vfr={vfr}; bframes={bframes}"
            );
        }
    }
}
#[test]
fn invalid_seek_anchor_falls_back_and_wrong_pts_is_rejected() {
    let d = tempfile::tempdir().unwrap();
    let source = fixture(d.path(), false, false);
    let mut frames = vec![];
    index(source.to_str().unwrap(), None, |m| {
        if let WorkerMessage::Frames { frames: batch } = m {
            frames.extend(batch)
        }
        Ok(())
    })
    .unwrap();
    let f = &frames[5];
    let bogus = SeekAnchor {
        ordinal: 3,
        pts: 999999,
        time_seconds: 0.3,
    };
    let path = d.path().join("fallback.png");
    decode(
        source.to_str().unwrap(),
        0,
        5,
        f.pts_ticks.as_ref().map(|p| p.parse().unwrap()),
        Some(&bogus),
        &path,
        None,
        &ExportOptions::default(),
    )
    .unwrap();
    assert!(path.exists());
    let error = decode(
        source.to_str().unwrap(),
        0,
        5,
        Some(-999),
        None,
        &d.path().join("invalid.png"),
        None,
        &ExportOptions::default(),
    )
    .unwrap_err();
    assert_eq!(error.code, "FRAME_MISMATCH");
    assert!(!d.path().join("invalid.png").exists());
}

#[test]
fn duplicate_timestamps_keep_distinct_ordinals_and_lossless_webp_pixels() {
    let d = tempfile::tempdir().unwrap();
    let path = d.path().join("duplicates.mkv");
    run(&[
        "-f",
        "lavfi",
        "-i",
        "testsrc2=size=160x120:rate=10:duration=1",
        "-vf",
        "setpts=floor(N/2)/(10*TB)",
        "-fps_mode",
        "passthrough",
        "-c:v",
        "ffv1",
        path.to_str().unwrap(),
    ]);
    let mut frames = vec![];
    index(path.to_str().unwrap(), None, |m| {
        if let WorkerMessage::Frames { frames: batch } = m {
            frames.extend(batch)
        }
        Ok(())
    })
    .unwrap();
    assert_eq!(frames.len(), 10);
    assert_eq!(frames[0].pts_ticks, frames[1].pts_ticks);
    assert_ne!(frames[0].ordinal, frames[1].ordinal);
    let mut images = vec![];
    for n in [0, 1] {
        let png = d.path().join(format!("{n}.png"));
        let webp = d.path().join(format!("{n}.webp"));
        let pts = frames[n].pts_ticks.as_ref().map(|p| p.parse().unwrap());
        decode(
            path.to_str().unwrap(),
            0,
            n as u64,
            pts,
            None,
            &png,
            None,
            &ExportOptions::default(),
        )
        .unwrap();
        decode(
            path.to_str().unwrap(),
            0,
            n as u64,
            pts,
            None,
            &webp,
            None,
            &ExportOptions {
                format: ExportFormat::Webp,
                ..ExportOptions::default()
            },
        )
        .unwrap();
        let a = image::open(png).unwrap().to_rgb8();
        let b = image::open(webp).unwrap().to_rgb8();
        assert_eq!(a, b);
        images.push(a);
    }
    assert_ne!(images[0], images[1]);
}

#[test]
fn pq_and_hlg_export_tagged_rgb16_and_tonemapped_sdr() {
    for transfer in ["smpte2084", "arib-std-b67"] {
        let d = tempfile::tempdir().unwrap();
        let source = d.path().join("hdr.mkv");
        run(&["-f","lavfi","-i","testsrc2=size=160x120:rate=2:duration=1","-vf",&format!("format=yuv420p10le,setparams=color_primaries=bt2020:color_trc={transfer}:colorspace=bt2020nc"),"-pix_fmt","yuv420p10le","-c:v","ffv1","-level","3","-color_primaries","bt2020","-color_trc",transfer,"-colorspace","bt2020nc","-color_range","tv",source.to_str().unwrap()]);
        let meta = frameiq_media::inspect(source.to_str().unwrap(), None).unwrap();
        assert_eq!(meta.streams[0].bit_depth, Some(10));
        let mut frames = Vec::new();
        index(source.to_str().unwrap(), None, |m| {
            if let WorkerMessage::Frames { frames: f } = m {
                frames.extend(f);
            }
            Ok(())
        })
        .unwrap();
        let pts = frames[0].pts_ticks.as_ref().map(|p| p.parse().unwrap());
        let preserved = d.path().join("preserve.png");
        decode(
            source.to_str().unwrap(),
            0,
            0,
            pts,
            None,
            &preserved,
            None,
            &ExportOptions {
                color_mode: ColorMode::Preserve16Bit,
                ..Default::default()
            },
        )
        .unwrap();
        assert!(matches!(
            image::open(&preserved).unwrap(),
            image::DynamicImage::ImageRgb16(_)
        ));
        let bytes = std::fs::read(&preserved).unwrap();
        let header = bytes.windows(8).find(|b| &b[..4] == b"cICP").unwrap();
        assert_eq!(
            &header[4..8],
            &[9, if transfer == "smpte2084" { 16 } else { 18 }, 0, 1]
        );
        let sdr = d.path().join("sdr.png");
        decode(
            source.to_str().unwrap(),
            0,
            0,
            pts,
            None,
            &sdr,
            Some(80),
            &ExportOptions::default(),
        )
        .unwrap();
        let image = image::open(&sdr).unwrap().to_rgb8();
        assert_eq!(image.dimensions(), (80, 60));
        assert!(image.pixels().any(|p| p[0] > 10 && p[0] < 240));
        assert!(image.pixels().any(|p| p[0] != p[1]));
    }
}
