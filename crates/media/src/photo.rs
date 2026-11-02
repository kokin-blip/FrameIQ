use crate::*;
use image::{DynamicImage, ImageDecoder, ImageFormat, ImageReader};
pub fn apply_photo_mask(
    path: &str,
    mask: &AssetRefV1,
    output: &str,
    r: &RegionRectV1,
) -> AppResult<WorkerMessage> {
    frameiq_analysis::presets::validate_rectangle(r)?;
    if hash_file(Path::new(&mask.path))?.0 != mask.sha256 {
        return Err(AppError::new("ASSET_CHANGED", "The mask changed."));
    }
    let mut reader = ImageReader::open(path)
        .map_err(media_error)?
        .with_guessed_format()
        .map_err(media_error)?;
    let mut limits = image::Limits::default();
    limits.max_alloc = Some(256 * 1024 * 1024);
    reader.limits(limits.clone());
    let source = reader.decode().map_err(media_error)?;
    let mut reader = ImageReader::open(&mask.path)
        .map_err(media_error)?
        .with_guessed_format()
        .map_err(media_error)?;
    reader.limits(limits);
    let mask = image::DynamicImage::from_decoder(reader.into_decoder().map_err(media_error)?)
        .map_err(media_error)?
        .to_luma8();
    let (w, h) = (source.width(), source.height());
    if mask.dimensions() != (w, h) {
        return Err(AppError::new(
            "MASK_SIZE",
            "The mask must match the source dimensions.",
        ));
    }
    let x = (r.x * w as f64).floor() as u32;
    let y = (r.y * h as f64).floor() as u32;
    let cw = ((r.width * w as f64).ceil() as u32).max(1).min(w - x);
    let ch = ((r.height * h as f64).ceil() as u32).max(1).min(h - y);
    let mut crop = source.crop_imm(x, y, cw, ch).to_rgba8();
    for (row_y, row) in crop.rows_mut().enumerate() {
        for (col, p) in row.enumerate() {
            p[3] = mask.get_pixel(x + col as u32, y + row_y as u32)[0];
        }
    }
    let file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(output)
        .map_err(media_error)?;
    let mut writer = std::io::BufWriter::new(file);
    DynamicImage::ImageRgba8(crop)
        .write_to(&mut writer, ImageFormat::Png)
        .map_err(media_error)?;
    writer.flush().map_err(media_error)?;
    writer.get_ref().sync_all().map_err(media_error)?;
    Ok(WorkerMessage::PhotoAssets {
        crop: AssetRefV1 {
            path: output.into(),
            sha256: hash_file(Path::new(output))?.0,
            width: cw,
            height: ch,
        },
        mask: None,
    })
}
pub fn crop_photo(
    path: &str,
    output: &str,
    mask_output: Option<&str>,
    r: &RegionRectV1,
    polygon: &[[f64; 2]],
) -> AppResult<WorkerMessage> {
    frameiq_analysis::presets::validate_rectangle(r)?;
    if polygon.len() > 128
        || polygon
            .iter()
            .flatten()
            .any(|n| !n.is_finite() || *n < 0. || *n > 1.)
        || (!polygon.is_empty() && polygon.len() < 3)
    {
        return Err(AppError::new(
            "INVALID_REGION",
            "Choose a bounded polygon inside the image.",
        ));
    }
    let mut reader = ImageReader::open(path)
        .map_err(media_error)?
        .with_guessed_format()
        .map_err(media_error)?;
    let mut limits = image::Limits::default();
    limits.max_alloc = Some(256 * 1024 * 1024);
    reader.limits(limits);
    let image = reader.decode().map_err(media_error)?;
    let (w, h) = (image.width(), image.height());
    let x = (r.x * w as f64).floor() as u32;
    let y = (r.y * h as f64).floor() as u32;
    let cw = ((r.width * w as f64).ceil() as u32).max(1).min(w - x);
    let ch = ((r.height * h as f64).ceil() as u32).max(1).min(h - y);
    let mut crop = image.crop_imm(x, y, cw, ch).to_rgba8();
    let mask = if let Some(out) = mask_output {
        let mut mask = image::GrayImage::new(w, h);
        for row in y..y + ch {
            let py = (row as f64 + 0.5) / h as f64;
            let mut edges = Vec::new();
            for i in 0..polygon.len() {
                let a = polygon[i];
                let b = polygon[(i + 1) % polygon.len()];
                if (a[1] > py) != (b[1] > py) {
                    edges.push((a[0] + (py - a[1]) * (b[0] - a[0]) / (b[1] - a[1])) * w as f64);
                }
            }
            edges.sort_by(f64::total_cmp);
            for col in x..x + cw {
                let inside = edges
                    .iter()
                    .filter(|edge| **edge > col as f64 + 0.5)
                    .count()
                    % 2
                    == 1;
                if inside {
                    mask.put_pixel(col, row, image::Luma([255]));
                } else {
                    crop.get_pixel_mut(col - x, row - y)[3] = 0;
                }
            }
        }
        let mut f = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(out)
            .map_err(media_error)?;
        let mut writer = std::io::BufWriter::new(&mut f);
        DynamicImage::ImageLuma8(mask)
            .write_to(&mut writer, ImageFormat::Png)
            .map_err(media_error)?;
        writer.flush().map_err(media_error)?;
        drop(writer);
        f.sync_all().map_err(media_error)?;
        Some(AssetRefV1 {
            path: out.into(),
            sha256: hash_file(Path::new(out))?.0,
            width: w,
            height: h,
        })
    } else {
        None
    };
    let mut f = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(output)
        .map_err(media_error)?;
    let mut writer = std::io::BufWriter::new(&mut f);
    DynamicImage::ImageRgba8(crop)
        .write_to(&mut writer, ImageFormat::Png)
        .map_err(media_error)?;
    writer.flush().map_err(media_error)?;
    drop(writer);
    f.sync_all().map_err(media_error)?;
    Ok(WorkerMessage::PhotoAssets {
        crop: AssetRefV1 {
            path: output.into(),
            sha256: hash_file(Path::new(output))?.0,
            width: cw,
            height: ch,
        },
        mask,
    })
}
pub fn decode_photo(path: &str, output: &str, max_width: Option<u32>) -> AppResult<WorkerMessage> {
    let before = std::fs::metadata(path).map_err(media_error)?;
    if before.len() > 256 * 1024 * 1024 {
        return Err(AppError::new(
            "PHOTO_LIMIT",
            "Choose an image smaller than 256 MiB.",
        ));
    }
    let mut reader = ImageReader::open(path)
        .map_err(media_error)?
        .with_guessed_format()
        .map_err(media_error)?;
    if !matches!(
        reader.format(),
        Some(ImageFormat::Jpeg | ImageFormat::Png | ImageFormat::WebP)
    ) {
        return Err(AppError::new(
            "PHOTO_FORMAT",
            "Choose a JPEG, PNG or WebP photo.",
        ));
    }
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(16384);
    limits.max_image_height = Some(16384);
    limits.max_alloc = Some(256 * 1024 * 1024);
    reader.limits(limits);
    let mut decoder = reader.into_decoder().map_err(media_error)?;
    let orientation = decoder.orientation().map_err(media_error)?;
    let mut image = DynamicImage::from_decoder(decoder).map_err(media_error)?;
    image.apply_orientation(orientation);
    if let Some(max) = max_width {
        if max == 0 || max > 16384 {
            return Err(AppError::new("PHOTO_LIMIT", "Invalid photo preview size."));
        }
        if image.width() > max || image.height() > max {
            image = image.resize(max, max, image::imageops::FilterType::Triangle);
        }
    }
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(output)
        .map_err(media_error)?;
    let mut writer = std::io::BufWriter::new(&mut file);
    image
        .write_to(&mut writer, ImageFormat::Png)
        .map_err(media_error)?;
    writer.flush().map_err(media_error)?;
    drop(writer);
    file.sync_all().map_err(media_error)?;
    let after = std::fs::metadata(path).map_err(media_error)?;
    if before.len() != after.len() || before.modified().ok() != after.modified().ok() {
        let _ = std::fs::remove_file(output);
        return Err(AppError::new(
            "SOURCE_CHANGED",
            "The photo changed while importing. Try again.",
        ));
    }
    let source_sha256 = hash_file(std::path::Path::new(path))?.0;
    Ok(WorkerMessage::Photo {
        asset: AssetRefV1 {
            path: output.into(),
            sha256: hash_file(std::path::Path::new(output))?.0,
            width: image.width(),
            height: image.height(),
        },
        source_sha256,
    })
}
