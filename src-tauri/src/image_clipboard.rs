use std::borrow::Cow;

use arboard::{Clipboard, ImageData};
use base64::{engine::general_purpose::STANDARD, Engine as _};
use serde::Deserialize;

const MAX_IMAGE_PIXELS: usize = 64 * 1024 * 1024;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClipboardImage {
    width: usize,
    height: usize,
    rgba_base64: String,
}

fn decode_image(image: ClipboardImage) -> Result<ImageData<'static>, String> {
    if image.width == 0 || image.height == 0 {
        return Err("Clipboard image dimensions must be positive".to_string());
    }

    let pixel_count = image
        .width
        .checked_mul(image.height)
        .ok_or_else(|| "Clipboard image dimensions are too large".to_string())?;
    if pixel_count > MAX_IMAGE_PIXELS {
        return Err("Clipboard image exceeds the pixel limit".to_string());
    }
    let expected_bytes = pixel_count
        .checked_mul(4)
        .ok_or_else(|| "Clipboard image dimensions are too large".to_string())?;
    let bytes = STANDARD
        .decode(image.rgba_base64)
        .map_err(|error| format!("Clipboard image is not valid base64: {error}"))?;
    if bytes.len() != expected_bytes {
        return Err(format!(
            "Clipboard image byte length does not match its dimensions: expected {expected_bytes}, got {}",
            bytes.len()
        ));
    }

    Ok(ImageData {
        width: image.width,
        height: image.height,
        bytes: Cow::Owned(bytes),
    })
}

#[tauri::command]
pub fn copy_image_to_clipboard(image: ClipboardImage) -> Result<(), String> {
    let image = decode_image(image)?;
    let mut clipboard = Clipboard::new()
        .map_err(|error| format!("Unable to access the Windows clipboard: {error}"))?;
    clipboard
        .set_image(image)
        .map_err(|error| format!("Unable to copy image to the Windows clipboard: {error}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_rgba_image_without_creating_a_file() {
        let image = decode_image(ClipboardImage {
            width: 1,
            height: 1,
            rgba_base64: "AQIDBA==".to_string(),
        })
        .expect("valid image");

        assert_eq!(image.width, 1);
        assert_eq!(image.height, 1);
        assert_eq!(image.bytes.as_ref(), &[1, 2, 3, 4]);
    }

    #[test]
    fn rejects_empty_and_oversized_dimensions() {
        let empty = decode_image(ClipboardImage {
            width: 0,
            height: 1,
            rgba_base64: String::new(),
        });
        assert!(empty.is_err());

        let oversized = decode_image(ClipboardImage {
            width: MAX_IMAGE_PIXELS + 1,
            height: 1,
            rgba_base64: String::new(),
        });
        assert!(oversized.is_err());
    }

    #[test]
    fn rejects_invalid_base64_and_mismatched_byte_lengths() {
        let invalid_base64 = decode_image(ClipboardImage {
            width: 1,
            height: 1,
            rgba_base64: "not base64".to_string(),
        });
        assert!(invalid_base64.is_err());

        let wrong_length = decode_image(ClipboardImage {
            width: 1,
            height: 1,
            rgba_base64: "AQID".to_string(),
        });
        assert!(wrong_length.is_err());
    }
}
