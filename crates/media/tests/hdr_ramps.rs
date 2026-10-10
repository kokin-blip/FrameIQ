//! Analytical gray ramps: the oracle never invokes the production color filters.
use frameiq_contracts::*;
use std::{path::Path, process::Command};

fn pq_nits(signal: f64) -> f64 {
    let p = signal.powf(1. / 78.84375);
    10000. * ((p - 0.8359375).max(0.) / (18.8515625 - 18.6875 * p)).powf(1. / 0.1593017578125)
}
fn hlg_nits(signal: f64) -> f64 {
    let scene = if signal <= 0.5 {
        signal * signal / 3.
    } else {
        (((signal - 0.55991073) / 0.17883277).exp() + 0.28466892) / 12.
    };
    1000. * scene.powf(1.2)
}
fn hable(x: f64) -> f64 {
    ((x * (0.15 * x + 0.05) + 0.004) / (x * (0.15 * x + 0.5) + 0.06)) - 0.02 / 0.3
}
fn srgb(x: f64) -> f64 {
    if x <= 0.0031308 {
        12.92 * x
    } else {
        1.055 * x.powf(1. / 2.4) - 0.055
    }
}

// Walk actual chunk boundaries, avoiding false positives inside compressed pixels.
fn chunk<'a>(bytes: &'a [u8], kind: &[u8; 4]) -> Option<&'a [u8]> {
    assert_eq!(&bytes[..8], b"\x89PNG\r\n\x1a\n");
    let mut offset = 8;
    while offset < bytes.len() {
        assert!(offset + 12 <= bytes.len());
        let len = u32::from_be_bytes(bytes[offset..offset + 4].try_into().unwrap()) as usize;
        let end = offset.checked_add(12 + len).unwrap();
        assert!(end <= bytes.len());
        if &bytes[offset + 4..offset + 8] == kind {
            return Some(&bytes[offset + 8..end - 4]);
        }
        offset = end;
    }
    None
}

#[test]
fn analytical_pq_hlg_ramps_preserve_codes_and_match_sdr_oracle() {
    let signals: [f64; 10] = [0., 0.05, 0.1, 0.25, 0.4, 0.5, 0.6, 0.75, 0.9, 1.];
    for (transfer, cicp, eotf) in [
        ("smpte2084", 16, pq_nits as fn(f64) -> f64),
        ("arib-std-b67", 18, hlg_nits as fn(f64) -> f64),
    ] {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("ramp.mkv");
        let codes: Vec<u16> = signals
            .iter()
            .map(|s| (4096. + s * 56064.).round() as u16)
            .collect();
        let mut luminance = codes[9].to_string();
        for i in (0..9).rev() {
            luminance = format!("if(lt(X,{}),{},{luminance})", (i + 1) * 16, codes[i]);
        }
        let fixture = format!("color=black:size=160x32:rate=1:duration=1,format=yuv444p16le,geq=lum='{luminance}':cb=32768:cr=32768,setparams=color_primaries=bt2020:color_trc={transfer}:colorspace=bt2020nc:range=limited");
        let binary = Path::new(env!("CARGO_MANIFEST_DIR")).join(if cfg!(windows) {
            "../../.native/ffmpeg/bin/ffmpeg.exe"
        } else {
            "../../.native/ffmpeg/bin/ffmpeg"
        });
        let result = Command::new(binary)
            .args([
                "-hide_banner",
                "-loglevel",
                "error",
                "-f",
                "lavfi",
                "-i",
                &fixture,
                "-frames:v",
                "1",
                "-c:v",
                "ffv1",
                "-level",
                "3",
                "-color_primaries",
                "bt2020",
                "-color_trc",
                transfer,
                "-colorspace",
                "bt2020nc",
                "-color_range",
                "tv",
                source.to_str().unwrap(),
            ])
            .output()
            .unwrap();
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        let mut frames = Vec::new();
        frameiq_media::index(source.to_str().unwrap(), None, |m| {
            if let WorkerMessage::Frames { frames: batch } = m {
                frames.extend(batch);
            }
            Ok(())
        })
        .unwrap();
        assert_eq!(frames.len(), 1);
        let pts = frames[0].pts_ticks.as_ref().map(|p| p.parse().unwrap());
        let preserve = dir.path().join("preserve.png");
        frameiq_media::decode(
            source.to_str().unwrap(),
            0,
            0,
            pts,
            None,
            &preserve,
            None,
            &ExportOptions {
                color_mode: ColorMode::Preserve16Bit,
                ..Default::default()
            },
        )
        .unwrap();
        let png = std::fs::read(&preserve).unwrap();
        assert_eq!(chunk(&png, b"IHDR").unwrap()[8], 16);
        assert_eq!(chunk(&png, b"cICP").unwrap(), &[9, cicp, 0, 1]);
        assert!(chunk(&png, b"sRGB").is_none());
        let image = image::open(preserve).unwrap().to_rgb16();
        for (i, code) in codes.iter().enumerate() {
            let expected = ((*code as f64 - 4096.) / 56064. * 65535.).round() as i32;
            for actual in image.get_pixel((i * 16 + 8) as u32, 16).0 {
                assert!(
                    (actual as i32 - expected).abs() <= 2,
                    "{transfer} stripe {i}: {actual} vs {expected}"
                );
            }
        }
        for peak in [None, Some(2000.)] {
            let output = dir.path().join(format!("sdr-{peak:?}.png"));
            frameiq_media::decode(
                source.to_str().unwrap(),
                0,
                0,
                pts,
                None,
                &output,
                None,
                &ExportOptions {
                    color_policy: Some(ColorPolicyV2 {
                        schema_version: 2,
                        source_peak_nits: peak,
                        source_interpretation: None,
                    }),
                    ..Default::default()
                },
            )
            .unwrap();
            let png = std::fs::read(&output).unwrap();
            assert_eq!(chunk(&png, b"IHDR").unwrap()[8], 8);
            assert_eq!(chunk(&png, b"cICP").unwrap(), &[1, 13, 0, 1]);
            assert!(chunk(&png, b"sRGB").is_some());
            let image = image::open(output).unwrap().to_rgb8();
            let mut previous = 0;
            for (i, code) in codes.iter().enumerate() {
                let signal = (*code as f64 - 4096.) / 56064.;
                let mapped = hable(eotf(signal) / 100.) / hable(peak.unwrap_or(1000.) / 100.);
                let expected = (srgb(mapped).clamp(0., 1.) * 255.).round() as i32;
                let pixel = image.get_pixel((i * 16 + 8) as u32, 16).0;
                assert!(pixel[0] >= previous);
                previous = pixel[0];
                for actual in pixel {
                    // One output code accommodates float arithmetic and encoder dithering.
                    assert!(
                        (actual as i32 - expected).abs() <= 1,
                        "{transfer} peak {peak:?} stripe {i}: {actual} vs {expected}"
                    );
                }
            }
        }
    }
}
