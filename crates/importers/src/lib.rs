//! Importers: raster images, SVG, and Adobe Animate XFL / FLA (CS5+ ZIP-based) documents.
//!
//! Every importer returns warnings for content it could not represent instead
//! of silently dropping it.

mod svg;
mod xfl;
mod xml;

use animara_document::{BitmapAsset, fresh_id};
use std::path::Path;
use std::sync::Arc;

pub use svg::{SvgImport, import_svg, import_svg_data};
pub use xfl::{XflImport, import_fla, import_xfl, import_xfl_dir};

#[derive(Debug, thiserror::Error)]
pub enum ImportError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("image error: {0}")]
    Image(#[from] image::ImageError),
    #[error("SVG error: {0}")]
    Svg(String),
    #[error("archive error: {0}")]
    Zip(#[from] zip::result::ZipError),
    #[error("XML error: {0}")]
    Xml(String),
    #[error("{0}")]
    Unsupported(String),
}

/// Import a raster image (PNG, JPEG, GIF, WebP, BMP) as a library bitmap.
/// Formats other than PNG/JPEG/GIF/WebP are re-encoded to PNG.
pub fn import_image(path: &Path) -> Result<BitmapAsset, ImportError> {
    let bytes = std::fs::read(path)?;
    let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| "bitmap".into());
    import_image_bytes(&name, bytes)
}

pub fn import_image_bytes(name: &str, bytes: Vec<u8>) -> Result<BitmapAsset, ImportError> {
    let fmt = image::guess_format(&bytes)?;
    let img = image::load_from_memory(&bytes)?;
    let (format, data) = match fmt {
        image::ImageFormat::Png => ("png", bytes),
        image::ImageFormat::Jpeg => ("jpg", bytes),
        image::ImageFormat::Gif => ("gif", bytes),
        image::ImageFormat::WebP => ("webp", bytes),
        _ => {
            let mut out = std::io::Cursor::new(Vec::new());
            img.write_to(&mut out, image::ImageFormat::Png)?;
            ("png", out.into_inner())
        }
    };
    Ok(BitmapAsset {
        id: fresh_id(),
        name: name.to_string(),
        width: img.width(),
        height: img.height(),
        format: format.into(),
        folder: String::new(),
        smoothing: true,
        data: Arc::new(data),
    })
}

/// Import every frame of an animated GIF as separate bitmaps (for frame-by-frame import).
pub fn import_gif_frames(path: &Path) -> Result<Vec<BitmapAsset>, ImportError> {
    use image::AnimationDecoder;
    let f = std::io::BufReader::new(std::fs::File::open(path)?);
    let dec = image::codecs::gif::GifDecoder::new(f)?;
    let frames = dec.into_frames().collect_frames()?;
    let base = path.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_else(|| "gif".into());
    frames
        .into_iter()
        .enumerate()
        .map(|(i, fr)| {
            let buf = fr.into_buffer();
            let mut out = std::io::Cursor::new(Vec::new());
            image::DynamicImage::ImageRgba8(buf).write_to(&mut out, image::ImageFormat::Png)?;
            import_image_bytes(&format!("{base} {:03}.png", i + 1), out.into_inner())
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn image_import_reencodes_bmp() {
        let img = image::RgbaImage::from_pixel(3, 2, image::Rgba([255, 0, 0, 255]));
        let mut bmp = std::io::Cursor::new(Vec::new());
        image::DynamicImage::ImageRgba8(img).write_to(&mut bmp, image::ImageFormat::Bmp).unwrap();
        let a = import_image_bytes("x.bmp", bmp.into_inner()).unwrap();
        assert_eq!((a.width, a.height, a.format.as_str()), (3, 2, "png"));
    }
}
