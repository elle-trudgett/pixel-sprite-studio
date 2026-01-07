use std::fs;

/// Check if a pixel at a given position is opaque in a base64-encoded image
pub fn is_pixel_opaque(base64_data: &str, x: u32, y: u32) -> bool {
    use base64::Engine;

    // Decode base64
    let png_bytes = match base64::engine::general_purpose::STANDARD.decode(base64_data) {
        Ok(bytes) => bytes,
        Err(_) => return false,
    };

    // Load image
    let img = match image::load_from_memory(&png_bytes) {
        Ok(img) => img.to_rgba8(),
        Err(_) => return false,
    };

    // Check bounds
    if x >= img.width() || y >= img.height() {
        return false;
    }

    // Check alpha channel (4th component of RGBA)
    let pixel = img.get_pixel(x, y);
    pixel[3] > 0
}

/// Import an image file and convert it to base64-encoded PNG
pub fn import_image_as_base64(path: &str) -> Result<String, String> {
    const MAX_TEXTURE_SIZE: u32 = 2048;

    let bytes = fs::read(path).map_err(|e| format!("Failed to read file: {}", e))?;

    // Verify it's a valid image
    let img = image::load_from_memory(&bytes).map_err(|e| format!("Invalid image: {}", e))?;

    // Resize if too large
    let (width, height) = (img.width(), img.height());
    let img = if width > MAX_TEXTURE_SIZE || height > MAX_TEXTURE_SIZE {
        let scale = (MAX_TEXTURE_SIZE as f32 / width as f32)
            .min(MAX_TEXTURE_SIZE as f32 / height as f32);
        let new_width = (width as f32 * scale) as u32;
        let new_height = (height as f32 * scale) as u32;
        // Note: Image will be resized to fit within 2048x2048
        img.resize(new_width, new_height, image::imageops::FilterType::Nearest)
    } else {
        img
    };

    // Re-encode as PNG to ensure consistent format
    let mut png_bytes = Vec::new();
    let mut cursor = std::io::Cursor::new(&mut png_bytes);
    img.write_to(&mut cursor, image::ImageFormat::Png)
        .map_err(|e| format!("Failed to encode PNG: {}", e))?;

    Ok(base64::Engine::encode(
        &base64::engine::general_purpose::STANDARD,
        &png_bytes,
    ))
}
