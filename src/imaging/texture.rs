use bevy_egui::egui;
use std::fs;

const MAX_TEXTURE_SIZE: u32 = 2048;

/// Decode base64 image data to an egui texture
pub fn decode_base64_to_texture(
    ctx: &egui::Context,
    name: &str,
    base64_data: &str,
) -> Result<egui::TextureHandle, String> {
    use base64::Engine;

    // Decode base64
    let png_bytes = base64::engine::general_purpose::STANDARD
        .decode(base64_data)
        .map_err(|e| format!("Failed to decode base64: {}", e))?;

    // Load image
    let img = image::load_from_memory(&png_bytes)
        .map_err(|e| format!("Failed to load image: {}", e))?;

    // Check if image needs to be resized
    let (width, height) = (img.width(), img.height());
    let img = if width > MAX_TEXTURE_SIZE || height > MAX_TEXTURE_SIZE {
        // Calculate new size maintaining aspect ratio
        let scale = (MAX_TEXTURE_SIZE as f32 / width as f32)
            .min(MAX_TEXTURE_SIZE as f32 / height as f32);
        let new_width = (width as f32 * scale) as u32;
        let new_height = (height as f32 * scale) as u32;
        img.resize(new_width, new_height, image::imageops::FilterType::Nearest)
    } else {
        img
    };

    // Convert to RGBA
    let rgba = img.to_rgba8();
    let size = [rgba.width() as usize, rgba.height() as usize];
    let pixels = rgba.into_raw();

    // Create egui ColorImage
    let color_image = egui::ColorImage::from_rgba_unmultiplied(size, &pixels);

    // Create texture
    Ok(ctx.load_texture(
        name,
        color_image,
        egui::TextureOptions::NEAREST, // Pixel art should use nearest neighbor
    ))
}

/// Create a yellow silhouette texture from base64 image data
/// All pixels become yellow (255, 255, 0) while preserving alpha
pub fn decode_base64_to_yellow_texture(
    ctx: &egui::Context,
    name: &str,
    base64_data: &str,
) -> Result<egui::TextureHandle, String> {
    use base64::Engine;

    // Decode base64
    let png_bytes = base64::engine::general_purpose::STANDARD
        .decode(base64_data)
        .map_err(|e| format!("Failed to decode base64: {}", e))?;

    // Load image
    let img = image::load_from_memory(&png_bytes)
        .map_err(|e| format!("Failed to load image: {}", e))?;

    // Check if image needs to be resized
    let (width, height) = (img.width(), img.height());
    let img = if width > MAX_TEXTURE_SIZE || height > MAX_TEXTURE_SIZE {
        let scale = (MAX_TEXTURE_SIZE as f32 / width as f32)
            .min(MAX_TEXTURE_SIZE as f32 / height as f32);
        let new_width = (width as f32 * scale) as u32;
        let new_height = (height as f32 * scale) as u32;
        img.resize(new_width, new_height, image::imageops::FilterType::Nearest)
    } else {
        img
    };

    // Convert to RGBA and make all pixels yellow (preserving alpha)
    let rgba = img.to_rgba8();
    let size = [rgba.width() as usize, rgba.height() as usize];
    let mut pixels = rgba.into_raw();

    // Convert every pixel to yellow while preserving alpha
    for chunk in pixels.chunks_mut(4) {
        // chunk is [R, G, B, A]
        chunk[0] = 255; // R
        chunk[1] = 255; // G
        chunk[2] = 0; // B
        // chunk[3] (alpha) stays the same
    }

    // Create egui ColorImage
    let color_image = egui::ColorImage::from_rgba_unmultiplied(size, &pixels);

    // Create texture
    Ok(ctx.load_texture(
        name,
        color_image,
        egui::TextureOptions::NEAREST,
    ))
}

/// Create a small JPG thumbnail for a reference image (for fallback when file is missing)
pub fn create_reference_thumbnail(path: &str, max_size: u32) -> Result<(String, (u32, u32)), String> {
    use base64::Engine;

    let bytes = fs::read(path).map_err(|e| format!("Failed to read file: {}", e))?;
    let img = image::load_from_memory(&bytes).map_err(|e| format!("Invalid image: {}", e))?;

    let original_size = (img.width(), img.height());

    // Scale down to fit within max_size
    let (width, height) = original_size;
    let img = if width > max_size || height > max_size {
        let scale = (max_size as f32 / width as f32).min(max_size as f32 / height as f32);
        let new_width = (width as f32 * scale) as u32;
        let new_height = (height as f32 * scale) as u32;
        img.resize(new_width, new_height, image::imageops::FilterType::Triangle)
    } else {
        img
    };

    // Encode as JPG with quality 80
    let mut jpg_bytes = Vec::new();
    let mut cursor = std::io::Cursor::new(&mut jpg_bytes);
    img.write_to(&mut cursor, image::ImageFormat::Jpeg)
        .map_err(|e| format!("Failed to encode JPEG: {}", e))?;

    Ok((
        base64::engine::general_purpose::STANDARD.encode(&jpg_bytes),
        original_size,
    ))
}

/// Calculate scale factor to fit an image within the canvas while preserving aspect ratio
pub fn calculate_fit_scale(image_size: (u32, u32), canvas_size: (u32, u32)) -> f32 {
    let scale_x = canvas_size.0 as f32 / image_size.0 as f32;
    let scale_y = canvas_size.1 as f32 / image_size.1 as f32;
    scale_x.min(scale_y)
}

/// Load reference image texture, falling back to thumbnail if file is missing
pub fn load_reference_texture(
    ctx: &egui::Context,
    file_path: &str,
    thumbnail_fallback: Option<&str>,
) -> Result<(egui::TextureHandle, (u32, u32), bool), String> {
    use base64::Engine;

    // Try loading from file first
    if let Ok(bytes) = fs::read(file_path) {
        if let Ok(img) = image::load_from_memory(&bytes) {
            let original_size = (img.width(), img.height());
            let rgba = img.to_rgba8();
            let size = [rgba.width() as usize, rgba.height() as usize];
            let pixels = rgba.into_raw();
            let color_image = egui::ColorImage::from_rgba_unmultiplied(size, &pixels);

            let texture = ctx.load_texture(
                file_path,
                color_image,
                egui::TextureOptions::LINEAR, // Reference images use linear filtering
            );
            return Ok((texture, original_size, false)); // false = not using fallback
        }
    }

    // Fall back to thumbnail
    if let Some(thumbnail_data) = thumbnail_fallback {
        let jpg_bytes = base64::engine::general_purpose::STANDARD
            .decode(thumbnail_data)
            .map_err(|e| format!("Failed to decode thumbnail: {}", e))?;

        let img = image::load_from_memory(&jpg_bytes)
            .map_err(|e| format!("Failed to load thumbnail: {}", e))?;

        let size = (img.width(), img.height());
        let rgba = img.to_rgba8();
        let pixels = rgba.into_raw();
        let color_image = egui::ColorImage::from_rgba_unmultiplied(
            [size.0 as usize, size.1 as usize],
            &pixels,
        );

        let texture = ctx.load_texture(
            &format!("{}_thumb", file_path),
            color_image,
            egui::TextureOptions::LINEAR,
        );
        return Ok((texture, size, true)); // true = using fallback
    }

    Err(format!(
        "File not found and no thumbnail available: {}",
        file_path
    ))
}
