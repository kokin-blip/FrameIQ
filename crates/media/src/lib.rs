mod color;
use ffmpeg_next as ffmpeg;
use frameiq_contracts::*;
use image::{DynamicImage, ImageFormat, RgbImage};
use sha2::{Digest, Sha256};
use std::{
    fs::File,
    io::{Read, Write},
    path::Path,
};

fn media_error(e: impl std::fmt::Display) -> AppError {
    AppError::new("MEDIA_ERROR", e.to_string())
}
pub fn hash_file(path: &Path) -> AppResult<(String, u64)> {
    let mut f = File::open(path)
        .map_err(|_| AppError::new("SOURCE_MISSING", "The source file cannot be opened."))?;
    let mut hash = Sha256::new();
    let mut buf = vec![0; 1024 * 1024];
    let mut bytes = 0;
    loop {
        let n = f.read(&mut buf).map_err(media_error)?;
        if n == 0 {
            break;
        }
        hash.update(&buf[..n]);
        bytes += n as u64;
    }
    Ok((format!("{:x}", hash.finalize()), bytes))
}
fn open(path: &str) -> AppResult<ffmpeg::format::context::Input> {
    if !Path::new(path).is_file() {
        return Err(AppError::new(
            "SOURCE_MISSING",
            "Select a local media file.",
        ));
    }
    ffmpeg::init().map_err(media_error)?;
    ffmpeg::util::log::set_level(ffmpeg::util::log::Level::Error);
    let mut dict = ffmpeg::Dictionary::new();
    dict.set("protocol_whitelist", "file,pipe");
    ffmpeg::format::input_with_dictionary(path, dict).map_err(media_error)
}
fn rotation(stream: &ffmpeg::Stream) -> i32 {
    for d in stream.side_data() {
        if d.kind() == ffmpeg::codec::packet::side_data::Type::DisplayMatrix && d.data().len() >= 36
        {
            let mut matrix = [0i32; 9];
            for (i, c) in d.data()[..36].chunks_exact(4).enumerate() {
                matrix[i] = i32::from_ne_bytes(c.try_into().unwrap());
            }
            let r = unsafe { ffmpeg::ffi::av_display_rotation_get(matrix.as_ptr()) };
            if r.is_finite() {
                return ((-r / 90.).round() as i32 * 90).rem_euclid(360);
            }
        }
    }
    stream
        .metadata()
        .get("rotate")
        .and_then(|v| v.parse::<i32>().ok())
        .unwrap_or(0)
        .rem_euclid(360)
}
pub fn inspect(path: &str, selected: Option<u32>) -> AppResult<MediaMetadata> {
    let input = open(path)?;
    let best = input
        .streams()
        .best(ffmpeg::media::Type::Video)
        .ok_or_else(|| AppError::new("UNSUPPORTED_CODEC", "No video stream was found."))?
        .index() as u32;
    let selected = selected.unwrap_or(best);
    let mut streams = Vec::new();
    for s in input
        .streams()
        .filter(|s| s.parameters().medium() == ffmpeg::media::Type::Video)
    {
        let decoder = ffmpeg::codec::context::Context::from_parameters(s.parameters())
            .map_err(media_error)?
            .decoder()
            .video()
            .map_err(media_error)?;
        let tb = s.time_base();
        let dur = s.duration();
        let sar = decoder.aspect_ratio();
        streams.push(StreamInfo {
            index: s.index() as u32,
            codec: s.parameters().id().name().into(),
            width: decoder.width(),
            height: decoder.height(),
            time_base_numerator: tb.numerator(),
            time_base_denominator: tb.denominator(),
            rotation: rotation(&s),
            sample_aspect_ratio: format!("{}:{}", sar.numerator(), sar.denominator()),
            color_space: format!("{:?}", decoder.color_space()),
            color_transfer: format!("{:?}", decoder.color_transfer_characteristic()),
            color_primaries: format!("{:?}", decoder.color_primaries()),
            bit_depth: unsafe {
                let desc = ffmpeg::ffi::av_pix_fmt_desc_get(decoder.format().into());
                if desc.is_null() {
                    None
                } else {
                    Some((*desc).comp[0].depth as u32)
                }
            },
            color_range: Some(format!("{:?}", decoder.color_range())),
            duration_seconds: if dur > 0 {
                Some(dur as f64 * f64::from(tb))
            } else {
                None
            },
        });
    }
    if !streams.iter().any(|s| s.index == selected) {
        return Err(AppError::new(
            "INVALID_STREAM",
            "The selected video stream does not exist.",
        ));
    }
    Ok(MediaMetadata {
        streams,
        selected_stream: selected,
        duration_seconds: if input.duration() > 0 {
            Some(input.duration() as f64 / ffmpeg::ffi::AV_TIME_BASE as f64)
        } else {
            None
        },
    })
}
fn walk(
    path: &str,
    stream: u32,
    anchor: Option<&SeekAnchor>,
    mut visit: impl FnMut(&ffmpeg::frame::Video, u64) -> AppResult<bool>,
) -> AppResult<()> {
    let mut input = open(path)?;
    let s = input
        .stream(stream as usize)
        .ok_or_else(|| AppError::new("INVALID_STREAM", "Invalid stream."))?;
    let mut context =
        ffmpeg::codec::context::Context::from_parameters(s.parameters()).map_err(media_error)?;
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
    context.set_threading(ffmpeg::codec::threading::Config::count(threads));
    let mut decoder = context.decoder().video().map_err(media_error)?;
    if let Some(a) = anchor {
        input
            .seek(
                (a.time_seconds * ffmpeg::ffi::AV_TIME_BASE as f64) as i64,
                ..,
            )
            .map_err(media_error)?;
        decoder.flush();
    }
    let color_defaults = (
        decoder.color_primaries(),
        decoder.color_transfer_characteristic(),
        decoder.color_space(),
        decoder.color_range(),
    );
    let mut ordinal = anchor.map_or(0, |a| a.ordinal);
    let mut first = true;
    let mut drain = |decoder: &mut ffmpeg::decoder::Video| -> AppResult<bool> {
        let mut f = ffmpeg::frame::Video::empty();
        loop {
            match decoder.receive_frame(&mut f) {
                Ok(()) => {
                    use ffmpeg::util::color::{Primaries, Range, Space, TransferCharacteristic};
                    if f.color_primaries() == Primaries::Unspecified {
                        f.set_color_primaries(color_defaults.0);
                    }
                    if f.color_transfer_characteristic() == TransferCharacteristic::Unspecified {
                        f.set_color_transfer_characteristic(color_defaults.1);
                    }
                    if f.color_space() == Space::Unspecified {
                        f.set_color_space(color_defaults.2);
                    }
                    if f.color_range() == Range::Unspecified {
                        f.set_color_range(color_defaults.3);
                    }
                    if first {
                        first = false;
                        if let Some(a) = anchor {
                            if f.pts().or(f.timestamp()) != Some(a.pts) || !f.is_key() {
                                return Err(AppError::new(
                                    "SEEK_MISMATCH",
                                    "Seek anchor could not be reproduced.",
                                ));
                            }
                        }
                    }
                    if f.width() == 0
                        || f.height() == 0
                        || f.width() as u64 * f.height() as u64 > 100_000_000
                    {
                        return Err(AppError::new(
                            "MEMORY_LIMIT",
                            "Decoded dimensions exceed the safety limit.",
                        ));
                    }
                    if visit(&f, ordinal)? {
                        return Ok(true);
                    }
                    ordinal += 1;
                }
                Err(ffmpeg::Error::Eof) => return Ok(false),
                Err(ffmpeg::Error::Other { errno }) if errno == ffmpeg::error::EAGAIN => {
                    return Ok(false)
                }
                Err(e) => return Err(media_error(e)),
            }
        }
    };
    let mut packet = ffmpeg::Packet::empty();
    loop {
        match packet.read(&mut input) {
            Ok(()) => {
                if packet.stream() == stream as usize {
                    decoder.send_packet(&packet).map_err(media_error)?;
                    if drain(&mut decoder)? {
                        return Ok(());
                    }
                }
            }
            Err(ffmpeg::Error::Eof) => break,
            Err(e) => return Err(media_error(e)),
        }
    }
    decoder.send_eof().map_err(media_error)?;
    drain(&mut decoder)?;
    Ok(())
}
pub fn index(
    path: &str,
    stream: Option<u32>,
    mut emit: impl FnMut(WorkerMessage) -> AppResult<()>,
) -> AppResult<()> {
    let meta = inspect(path, stream)?;
    let selected = meta.selected_stream;
    let info = meta.streams.iter().find(|s| s.index == selected).unwrap();
    let tb = info.time_base_numerator as f64 / info.time_base_denominator as f64;
    emit(WorkerMessage::Metadata { metadata: meta })?;
    let (sha256, bytes) = hash_file(Path::new(path))?;
    emit(WorkerMessage::Revision { sha256, bytes })?;
    let mut batch = Vec::with_capacity(128);
    let mut pending: Option<IndexedFrame> = None;
    let mut synthetic_time = 0.;
    walk(path, selected, None, |f, ordinal| {
        let pts = f.pts().or(f.timestamp());
        let duration = f.packet().duration;
        let dt = if duration > 0 {
            Some(duration as f64 * tb)
        } else {
            None
        };
        let time = pts.map(|p| p as f64 * tb).unwrap_or(synthetic_time);
        if let Some(mut previous) = pending.take() {
            if previous.duration_seconds.is_none() && time > previous.time_seconds {
                previous.duration_seconds = Some(time - previous.time_seconds);
            }
            batch.push(previous);
        }
        synthetic_time = time + dt.unwrap_or(1. / 30.);
        pending = Some(IndexedFrame {
            ordinal: ordinal.to_string(),
            pts_ticks: pts.map(|p| p.to_string()),
            time_seconds: time,
            duration_seconds: dt,
            timestamp_origin: if f.pts().is_some() {
                TimestampOrigin::Source
            } else if f.timestamp().is_some() {
                TimestampOrigin::BestEffort
            } else {
                TimestampOrigin::Synthetic
            },
            key_frame: f.is_key(),
        });
        if batch.len() >= 128 {
            emit(WorkerMessage::Frames {
                frames: std::mem::take(&mut batch),
            })?;
        }
        Ok(false)
    })?;
    if let Some(f) = pending {
        batch.push(f);
    }
    if !batch.is_empty() {
        emit(WorkerMessage::Frames { frames: batch })?;
    }
    emit(WorkerMessage::Complete)
}
fn rgb(
    frame: &ffmpeg::frame::Video,
    max_width: Option<u32>,
    rotate: i32,
) -> AppResult<DynamicImage> {
    use ffmpeg::util::color::{Primaries, Range, Space, TransferCharacteristic};
    if matches!(
        frame.color_transfer_characteristic(),
        TransferCharacteristic::SMPTE2084 | TransferCharacteristic::ARIB_STD_B67
    ) || matches!(frame.color_primaries(), Primaries::BT2020)
    {
        return Err(AppError::new("HDR_UNSUPPORTED","This build does not yet support validated HDR/wide-gamut conversion. Export is blocked to avoid clipping the source."));
    }
    let w = max_width.unwrap_or(frame.width()).min(frame.width()).max(1);
    let h = ((frame.height() as u64 * w as u64) / frame.width() as u64).max(1) as u32;
    let mut scaler = ffmpeg::software::scaling::Context::get(
        frame.format(),
        frame.width(),
        frame.height(),
        ffmpeg::format::Pixel::RGB24,
        w,
        h,
        ffmpeg::software::scaling::Flags::BICUBIC,
    )
    .map_err(media_error)?;
    let cs = match frame.color_space() {
        Space::BT709 => ffmpeg::ffi::SWS_CS_ITU709,
        Space::BT2020NCL | Space::BT2020CL => ffmpeg::ffi::SWS_CS_BT2020,
        _ => ffmpeg::ffi::SWS_CS_ITU601,
    };
    unsafe {
        let coeff = ffmpeg::ffi::sws_getCoefficients(cs);
        let result = ffmpeg::ffi::sws_setColorspaceDetails(
            scaler.as_mut_ptr(),
            coeff,
            if frame.color_range() == Range::JPEG {
                1
            } else {
                0
            },
            coeff,
            1,
            0,
            1 << 16,
            1 << 16,
        );
        if result < 0 {
            return Err(AppError::new(
                "COLOR_CONVERSION",
                "Unable to configure color conversion.",
            ));
        }
    }
    let mut output = ffmpeg::frame::Video::empty();
    scaler.run(frame, &mut output).map_err(media_error)?;
    let mut bytes = vec![0u8; w as usize * h as usize * 3];
    for y in 0..h as usize {
        bytes[y * w as usize * 3..(y + 1) * w as usize * 3].copy_from_slice(
            &output.data(0)[y * output.stride(0)..y * output.stride(0) + w as usize * 3],
        );
    }
    let image = DynamicImage::ImageRgb8(
        RgbImage::from_raw(w, h, bytes)
            .ok_or_else(|| AppError::new("MEDIA_ERROR", "Invalid RGB image."))?,
    );
    Ok(match rotate {
        90 => image.rotate90(),
        180 => image.rotate180(),
        270 => image.rotate270(),
        _ => image,
    })
}
// The flat fields mirror the versioned worker request; native handles stay internal.
#[allow(clippy::too_many_arguments)]
pub fn decode(
    path: &str,
    stream: u32,
    ordinal: u64,
    expected_pts: Option<i64>,
    anchor: Option<&SeekAnchor>,
    output: &Path,
    max_width: Option<u32>,
    options: &ExportOptions,
) -> AppResult<WorkerMessage> {
    if options.color_mode == ColorMode::Preserve16Bit && options.format != ExportFormat::Png {
        return Err(AppError::new(
            "INVALID_EXPORT",
            "16-bit output requires PNG.",
        ));
    }
    if !(1..=100).contains(&options.jpeg_quality) {
        return Err(AppError::new(
            "INVALID_EXPORT",
            "JPEG quality must be between 1 and 100.",
        ));
    }
    let info = inspect(path, Some(stream))?;
    let rot = info
        .streams
        .iter()
        .find(|s| s.index == stream)
        .unwrap()
        .rotation;
    let mut selected = None;
    let mut run = |a: Option<&SeekAnchor>| {
        walk(path, stream, a, |f, n| {
            if n == ordinal {
                if f.pts().or(f.timestamp()) != expected_pts {
                    return Err(AppError::new(
                        "FRAME_MISMATCH",
                        "Decoded frame no longer matches its index.",
                    ));
                }
                selected = Some(color::convert(f, max_width, rot, options)?);
                return Ok(true);
            }
            Ok(false)
        })
    };
    if let Some(a) = anchor {
        if run(Some(a)).is_err() {
            run(None)?;
        }
    } else {
        run(None)?;
    }
    let converted = selected.ok_or_else(|| {
        AppError::new(
            "FRAME_MISSING",
            "The requested frame is beyond the decoded stream.",
        )
    })?;
    let image = &converted.image;
    let parent = output
        .parent()
        .ok_or_else(|| AppError::new("INVALID_EXPORT", "Invalid output directory."))?;
    std::fs::create_dir_all(parent).map_err(media_error)?;
    let mut temp = tempfile::NamedTempFile::new_in(parent).map_err(media_error)?;
    match options.format {
        ExportFormat::Png => color::write_png(&mut temp, &converted)?,
        ExportFormat::Webp => image
            .write_to(&mut temp, ImageFormat::WebP)
            .map_err(media_error)?,
        ExportFormat::Jpeg => {
            image::codecs::jpeg::JpegEncoder::new_with_quality(&mut temp, options.jpeg_quality)
                .encode_image(image)
                .map_err(media_error)?
        }
    };
    temp.flush().map_err(media_error)?;
    temp.as_file().sync_all().map_err(media_error)?;
    temp.persist_noclobber(output).map_err(media_error)?;
    let (sha256, _) = hash_file(output)?;
    Ok(WorkerMessage::Image {
        path: output.to_string_lossy().into(),
        width: image.width(),
        height: image.height(),
        sha256,
        conversion: converted.report,
    })
}

/// Decode one ordered pass without RGB-converting frames outside the requested targets.
pub fn analyze_batch(
    path: &str,
    stream: u32,
    targets: &[AnalysisTarget],
    mut emit: impl FnMut(WorkerMessage) -> AppResult<()>,
) -> AppResult<()> {
    if targets.len() > 20000 || targets.windows(2).any(|p| p[0].ordinal >= p[1].ordinal) {
        return Err(AppError::new(
            "INVALID_ANALYSIS",
            "Analysis targets must be sorted, unique, and bounded.",
        ));
    }
    let meta = inspect(path, Some(stream))?;
    let rotation = meta
        .streams
        .iter()
        .find(|s| s.index == stream)
        .unwrap()
        .rotation;
    let mut next = 0;
    if !targets.is_empty() {
        walk(path, stream, None, |f, n| {
            if n == targets[next].ordinal {
                if f.pts().or(f.timestamp()) != targets[next].expected_pts {
                    return Err(AppError::new(
                        "FRAME_MISMATCH",
                        "Analysis frame differs from the index.",
                    ));
                }
                let image =
                    color::convert(f, Some(640), rotation, &ExportOptions::default())?.image;
                emit(WorkerMessage::Quality {
                    ordinal: n.to_string(),
                    metrics: frameiq_analysis::measure(&image),
                })?;
                next += 1;
            }
            Ok(next == targets.len())
        })?;
    }
    if next != targets.len() {
        return Err(AppError::new(
            "FRAME_MISSING",
            "Not all requested analysis frames could be decoded.",
        ));
    }
    emit(WorkerMessage::Complete)
}
