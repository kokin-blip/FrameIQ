use super::*;
#[allow(non_snake_case, non_camel_case_types, dead_code)]
mod metadata {
    include!(concat!(env!("OUT_DIR"), "/hdr_metadata.rs"));
}

use ffmpeg::util::color::{Primaries, TransferCharacteristic};
pub struct Converted {
    pub image: DynamicImage,
    pub cicp: Option<[u8; 4]>,
    pub mastering: Option<Vec<u8>>,
    pub light: Option<Vec<u8>>,
    pub report: serde_json::Value,
}
fn filter(frame: &ffmpeg::frame::Video, spec: &str) -> AppResult<ffmpeg::frame::Video> {
    let mut graph = ffmpeg::filter::Graph::new();
    let budget = (std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(1)
        / 2)
    .clamp(1, 4);
    let threads = std::env::var("FRAMEIQ_CPU_THREADS")
        .ok()
        .and_then(|s| s.parse::<usize>().ok())
        .unwrap_or(budget)
        .clamp(1, budget);
    // Match the decoder's assigned compute slot; avoid automatic filter thread pools.
    unsafe {
        (*graph.as_mut_ptr()).nb_threads = threads as i32;
    }

    let buffer = ffmpeg::filter::find("buffer").ok_or_else(|| {
        AppError::new(
            "COLOR_ENGINE_MISSING",
            "Rebuild the media engine with color filters.",
        )
    })?;
    let sink =
        ffmpeg::filter::find("buffersink").ok_or_else(|| media_error("Missing buffersink"))?;
    let format: ffmpeg::ffi::AVPixelFormat = frame.format().into();
    graph
        .add(
            &buffer,
            "in",
            &format!(
                "video_size={}x{}:pix_fmt={}:time_base=1/1000000:pixel_aspect=1/1",
                frame.width(),
                frame.height(),
                format as i32
            ),
        )
        .map_err(media_error)?;
    graph.add(&sink, "out", "").map_err(media_error)?;
    graph
        .output("in", 0)
        .map_err(media_error)?
        .input("out", 0)
        .map_err(media_error)?
        .parse(spec)
        .map_err(media_error)?;
    graph.validate().map_err(media_error)?;
    graph
        .get("in")
        .unwrap()
        .source()
        .add(frame)
        .map_err(media_error)?;
    let mut output = ffmpeg::frame::Video::empty();
    graph
        .get("out")
        .unwrap()
        .sink()
        .frame(&mut output)
        .map_err(media_error)?;
    Ok(output)
}
pub fn convert(
    frame: &ffmpeg::frame::Video,
    max_width: Option<u32>,
    rotation: i32,
    options: &ExportOptions,
) -> AppResult<Converted> {
    if let Some(policy) = &options.color_policy {
        if policy.schema_version != 2
            || policy
                .source_peak_nits
                .is_some_and(|x| !x.is_finite() || !(100.0..=10000.0).contains(&x))
        {
            return Err(AppError::new(
                "INVALID_COLOR_POLICY",
                "Use color policy 2 and a source peak between 100 and 10,000 nits.",
            ));
        }
    }
    let mut interpreted = frame.clone();
    if let Some(name) = options
        .color_policy
        .as_ref()
        .and_then(|p| p.source_interpretation.as_deref())
    {
        use ffmpeg::util::color::{Range, Space};
        let (p, t, m) = match name {
            "bt709" => (
                Primaries::BT709,
                TransferCharacteristic::BT709,
                Space::BT709,
            ),
            "bt601_625" => (
                Primaries::BT470BG,
                TransferCharacteristic::BT709,
                Space::BT470BG,
            ),
            "bt601_525" => (
                Primaries::SMPTE170M,
                TransferCharacteristic::SMPTE170M,
                Space::SMPTE170M,
            ),
            "hdr10" => (
                Primaries::BT2020,
                TransferCharacteristic::SMPTE2084,
                Space::BT2020NCL,
            ),
            "hlg" => (
                Primaries::BT2020,
                TransferCharacteristic::ARIB_STD_B67,
                Space::BT2020NCL,
            ),
            _ => {
                return Err(AppError::new(
                    "INVALID_COLOR_POLICY",
                    "Unknown source color interpretation.",
                ))
            }
        };
        interpreted.set_color_primaries(p);
        interpreted.set_color_transfer_characteristic(t);
        interpreted.set_color_space(m);
        if interpreted.color_range() == Range::Unspecified {
            interpreted.set_color_range(Range::MPEG);
        }
    }
    let frame = &interpreted;
    let hdr = matches!(
        frame.color_transfer_characteristic(),
        TransferCharacteristic::SMPTE2084 | TransferCharacteristic::ARIB_STD_B67
    );
    let wide = frame.color_primaries() == Primaries::BT2020;
    let preserve = options.color_mode == ColorMode::Preserve16Bit;
    if !preserve && !hdr && !wide && options.color_policy.is_none() {
        return Ok(Converted {
            image: super::rgb(frame, max_width, rotation)?,
            cicp: None,
            mastering: None,
            light: None,
            report: serde_json::json!({"policy":"legacy_sdr","resize":max_width,"rotation":rotation}),
        });
    }
    let prim: ffmpeg::ffi::AVColorPrimaries = frame.color_primaries().into();
    let transfer: ffmpeg::ffi::AVColorTransferCharacteristic =
        frame.color_transfer_characteristic().into();
    if prim as u32 == 2
        || transfer as u32 == 2
        || frame.color_space() == ffmpeg::util::color::Space::Unspecified
    {
        return Err(AppError::new("COLOR_METADATA_REQUIRED",format!("This conversion requires explicit source primaries, transfer, and matrix metadata (received {:?}/{:?}/{:?}).",frame.color_primaries(),frame.color_transfer_characteristic(),frame.color_space())));
    }
    // Reject dynamic Dolby Vision interpretation; no promise of preserving its metadata.
    if frame
        .side_data(ffmpeg::util::frame::side_data::Type::DOVI_RPU_BUFFER)
        .is_some()
    {
        return Err(AppError::new(
            "DOLBY_VISION_UNSUPPORTED",
            "Dolby Vision dynamic metadata is not supported by this color policy.",
        ));
    }
    let peak = options
        .color_policy
        .as_ref()
        .and_then(|p| p.source_peak_nits)
        .unwrap_or_else(|| source_peak(frame).unwrap_or(1000.));
    let spec = if preserve {
        "zscale=m=gbr:r=full,format=gbrp16le,format=rgb48le".into()
    } else if hdr {
        // zimg HLG display EOTF includes its 1.2 system gamma at the 1000-nit reference.
        format!("zscale=t=linear:npl=100,format=gbrpf32le,zscale=p=bt709,tonemap=tonemap=hable:desat=0:peak={},zscale=t=iec61966-2-1:m=gbr:r=full,format=gbrp,format=rgb24",peak/100.)
    } else {
        "zscale=p=bt709:t=iec61966-2-1:m=gbr:r=full,format=gbrp,format=rgb24".into()
    };
    let transformed = filter(frame, &spec)?;
    let w = transformed.width();
    let h = transformed.height();
    let image = if preserve {
        let mut values = Vec::with_capacity(w as usize * h as usize * 3);
        for y in 0..h as usize {
            for b in transformed.data(0)
                [y * transformed.stride(0)..y * transformed.stride(0) + w as usize * 6]
                .chunks_exact(2)
            {
                values.push(u16::from_le_bytes([b[0], b[1]]));
            }
        }
        DynamicImage::ImageRgb16(
            image::ImageBuffer::from_raw(w, h, values)
                .ok_or_else(|| media_error("Invalid RGB16"))?,
        )
    } else {
        let mut bytes = Vec::with_capacity(w as usize * h as usize * 3);
        for y in 0..h as usize {
            bytes.extend_from_slice(
                &transformed.data(0)
                    [y * transformed.stride(0)..y * transformed.stride(0) + w as usize * 3],
            );
        }
        DynamicImage::ImageRgb8(
            RgbImage::from_raw(w, h, bytes).ok_or_else(|| media_error("Invalid RGB8"))?,
        )
    };
    let image = if !preserve {
        if let Some(limit) = max_width {
            image.resize(
                limit.min(w).max(1),
                h,
                image::imageops::FilterType::Triangle,
            )
        } else {
            image
        }
    } else {
        image
    };
    let image = match rotation {
        90 => image.rotate90(),
        180 => image.rotate180(),
        270 => image.rotate270(),
        _ => image,
    };
    let (mastering, light) = if preserve {
        hdr_chunks(frame)
    } else {
        (None, None)
    };
    let peak_origin = if options
        .color_policy
        .as_ref()
        .and_then(|p| p.source_peak_nits)
        .is_some()
    {
        "user"
    } else if source_peak(frame).is_some() {
        "mastering_metadata"
    } else {
        "fallback"
    };
    let report = serde_json::json!({"policy":if preserve{"rgb16_preserve"}else{"srgb_sdr"},"sourcePrimaries":format!("{:?}",frame.color_primaries()),"sourceTransfer":format!("{:?}",frame.color_transfer_characteristic()),"sourceMatrix":format!("{:?}",frame.color_space()),"sourceRange":format!("{:?}",frame.color_range()),"sourceInterpretation":options.color_policy.as_ref().and_then(|p|p.source_interpretation.as_deref()),"toneMapping":if hdr && !preserve{Some("Hable; target 100 nits; desaturation disabled")}else{None},"sourcePeakNits":if hdr{Some(peak)}else{None},"sourcePeakOrigin":if hdr{Some(peak_origin)}else{None},"hlgReferenceDisplay":if frame.color_transfer_characteristic()==TransferCharacteristic::ARIB_STD_B67{Some("zimg HLG system gamma 1.2; 1000-nit reference display")}else{None},"rotation":rotation,"masteringChunkHex":mastering.as_ref().map(|b|b.iter().map(|n|format!("{n:02x}")).collect::<String>()),"lightLevelChunkHex":light.as_ref().map(|b|b.iter().map(|n|format!("{n:02x}")).collect::<String>()),"filter":spec});
    Ok(Converted {
        image,
        report,
        cicp: Some(if preserve {
            [prim as u8, transfer as u8, 0, 1]
        } else {
            [1, 13, 0, 1]
        }),
        mastering,
        light,
    })
}
fn source_peak(frame: &ffmpeg::frame::Video) -> Option<f64> {
    frame
        .side_data(ffmpeg::util::frame::side_data::Type::MasteringDisplayMetadata)
        .and_then(|s| {
            if s.data().len() < std::mem::size_of::<metadata::AVMasteringDisplayMetadata>() {
                return None;
            }
            let m = unsafe {
                std::ptr::read_unaligned(
                    s.data()
                        .as_ptr()
                        .cast::<metadata::AVMasteringDisplayMetadata>(),
                )
            };
            if m.has_luminance == 0 || m.max_luminance.den <= 0 {
                return None;
            }
            let peak = m.max_luminance.num as f64 / m.max_luminance.den as f64;
            (100.0..=10000.0).contains(&peak).then_some(peak)
        })
}
fn hdr_chunks(frame: &ffmpeg::frame::Video) -> (Option<Vec<u8>>, Option<Vec<u8>>) {
    let mastering = frame
        .side_data(ffmpeg::util::frame::side_data::Type::MasteringDisplayMetadata)
        .and_then(|s| {
            if s.data().len() < std::mem::size_of::<metadata::AVMasteringDisplayMetadata>() {
                return None;
            }
            let m = unsafe {
                std::ptr::read_unaligned(
                    s.data()
                        .as_ptr()
                        .cast::<metadata::AVMasteringDisplayMetadata>(),
                )
            };
            if m.has_primaries == 0 || m.has_luminance == 0 {
                return None;
            }
            let mut out = Vec::new();
            for point in m
                .display_primaries
                .iter()
                .chain(std::iter::once(&m.white_point))
            {
                for r in point {
                    if r.den <= 0 {
                        return None;
                    }
                    out.extend_from_slice(
                        &((r.num as f64 / r.den as f64 * 50000.)
                            .round()
                            .clamp(0., 65535.) as u16)
                            .to_be_bytes(),
                    );
                }
            }
            for r in [m.max_luminance, m.min_luminance] {
                if r.den <= 0 {
                    return None;
                }
                out.extend_from_slice(
                    &((r.num as f64 / r.den as f64 * 10000.).round().max(0.) as u32).to_be_bytes(),
                );
            }
            Some(out)
        });
    let light = frame
        .side_data(ffmpeg::util::frame::side_data::Type::ContentLightLevel)
        .and_then(|s| {
            if s.data().len() < std::mem::size_of::<metadata::AVContentLightMetadata>() {
                return None;
            }
            let m = unsafe {
                std::ptr::read_unaligned(
                    s.data().as_ptr().cast::<metadata::AVContentLightMetadata>(),
                )
            };
            let mut out = Vec::new();
            for n in [m.MaxCLL, m.MaxFALL] {
                out.extend_from_slice(&(n.saturating_mul(10000)).to_be_bytes());
            }
            Some(out)
        });
    (mastering, light)
}
pub fn write_png(writer: &mut impl Write, converted: &Converted) -> AppResult<()> {
    let image = &converted.image;
    let mut encoder = png::Encoder::new(writer, image.width(), image.height());
    encoder.set_color(png::ColorType::Rgb);
    let is16 = matches!(image, DynamicImage::ImageRgb16(_));
    encoder.set_depth(if is16 {
        png::BitDepth::Sixteen
    } else {
        png::BitDepth::Eight
    });
    if converted.cicp == Some([1, 13, 0, 1]) {
        encoder.set_source_srgb(png::SrgbRenderingIntent::RelativeColorimetric);
    }
    let mut writer = encoder.write_header().map_err(media_error)?;
    if let Some(cicp) = converted.cicp {
        writer
            .write_chunk(png::chunk::ChunkType(*b"cICP"), &cicp)
            .map_err(media_error)?;
    }
    if let Some(data) = &converted.mastering {
        writer
            .write_chunk(png::chunk::ChunkType(*b"mDCV"), data)
            .map_err(media_error)?;
    }
    if let Some(data) = &converted.light {
        writer
            .write_chunk(png::chunk::ChunkType(*b"cLLI"), data)
            .map_err(media_error)?;
    }
    let bytes = if is16 {
        image
            .to_rgb16()
            .as_raw()
            .iter()
            .flat_map(|n| n.to_be_bytes())
            .collect()
    } else {
        image.to_rgb8().into_raw()
    };
    writer.write_image_data(&bytes).map_err(media_error)?;
    writer.finish().map_err(media_error)
}
