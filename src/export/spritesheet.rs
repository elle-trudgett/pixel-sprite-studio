use std::fs;
use std::path::PathBuf;

use crate::model::{Animation, Project};
use crate::state::AppState;

/// Render a single frame to an RGBA image buffer
pub fn render_frame_to_image(
    project: &Project,
    animation: &Animation,
    frame_idx: usize,
    canvas_size: (u32, u32),
) -> Result<image::RgbaImage, String> {
    let frame = animation
        .frames
        .get(frame_idx)
        .ok_or_else(|| format!("Frame {} not found", frame_idx))?;

    let (canvas_w, canvas_h) = canvas_size;
    let mut canvas = image::RgbaImage::new(canvas_w, canvas_h);

    // Draw each placed part (in order - later parts on top)
    for placed in &frame.placed_parts {
        // Skip invisible layers
        if !placed.visible {
            continue;
        }

        // Find the part's image data (look up by character_id for stability)
        let image_data = project
            .get_character_by_id(placed.character_id)
            .and_then(|c| c.get_part(&placed.part_name))
            .and_then(|p| p.states.iter().find(|s| s.name == placed.state_name))
            .and_then(|s| s.rotations.get(&placed.rotation))
            .and_then(|r| r.image_data.as_ref());

        if let Some(base64_data) = image_data {
            // Decode the image
            use base64::Engine;
            let png_bytes = base64::engine::general_purpose::STANDARD
                .decode(base64_data)
                .map_err(|e| format!("Base64 decode error: {}", e))?;

            let part_img = image::load_from_memory(&png_bytes)
                .map_err(|e| format!("Image load error: {}", e))?
                .to_rgba8();

            // Composite onto canvas at the specified position
            let x = placed.position.0.round() as i32;
            let y = placed.position.1.round() as i32;

            for (px, py, pixel) in part_img.enumerate_pixels() {
                let dest_x = x + px as i32;
                let dest_y = y + py as i32;

                // Bounds check
                if dest_x >= 0
                    && dest_x < canvas_w as i32
                    && dest_y >= 0
                    && dest_y < canvas_h as i32
                {
                    let dest_x = dest_x as u32;
                    let dest_y = dest_y as u32;

                    // Alpha blending
                    let src = pixel;
                    let dst = canvas.get_pixel_mut(dest_x, dest_y);

                    let src_a = src[3] as f32 / 255.0;
                    let dst_a = dst[3] as f32 / 255.0;
                    let out_a = src_a + dst_a * (1.0 - src_a);

                    if out_a > 0.0 {
                        for i in 0..3 {
                            let src_c = src[i] as f32 / 255.0;
                            let dst_c = dst[i] as f32 / 255.0;
                            let out_c = (src_c * src_a + dst_c * dst_a * (1.0 - src_a)) / out_a;
                            dst[i] = (out_c * 255.0).round() as u8;
                        }
                        dst[3] = (out_a * 255.0).round() as u8;
                    }
                }
            }
        }
    }

    Ok(canvas)
}

/// Export the current animation as a spritesheet
pub fn export_current_animation(
    state: &AppState,
    output_path: &str,
) -> Result<(String, String), String> {
    let project = state.project.as_ref().ok_or("No project loaded")?;
    let char_name = state
        .active_character
        .as_ref()
        .ok_or("No character selected")?;
    let character = project.get_character(char_name).ok_or("Character not found")?;
    let animation = character
        .animations
        .get(state.current_animation)
        .ok_or("Animation not found")?;

    if animation.frames.is_empty() {
        return Err("Animation has no frames".to_string());
    }

    let (canvas_w, canvas_h) = character.canvas_size;
    let frame_count = animation.frames.len();

    // Calculate spritesheet dimensions (horizontal strip for small counts, grid for larger)
    let (cols, rows) = if frame_count <= 8 {
        (frame_count, 1)
    } else {
        let cols = (frame_count as f32).sqrt().ceil() as usize;
        let rows = (frame_count + cols - 1) / cols;
        (cols, rows)
    };

    let sheet_w = cols as u32 * canvas_w;
    let sheet_h = rows as u32 * canvas_h;
    let mut spritesheet = image::RgbaImage::new(sheet_w, sheet_h);

    // Render each frame and place it in the spritesheet
    let mut frame_metadata = Vec::new();
    let canvas_size = character.canvas_size;
    for (i, frame) in animation.frames.iter().enumerate() {
        let frame_img = render_frame_to_image(project, animation, i, canvas_size)?;

        let col = i % cols;
        let row = i / cols;
        let x = col as u32 * canvas_w;
        let y = row as u32 * canvas_h;

        // Copy frame to spritesheet
        for (px, py, pixel) in frame_img.enumerate_pixels() {
            spritesheet.put_pixel(x + px, y + py, *pixel);
        }

        frame_metadata.push(serde_json::json!({
            "x": x,
            "y": y,
            "width": canvas_w,
            "height": canvas_h,
            "duration_ms": frame.duration_ms
        }));
    }

    // Ensure output path ends with .png
    let png_path = if output_path.to_lowercase().ends_with(".png") {
        output_path.to_string()
    } else {
        format!("{}.png", output_path)
    };

    // Save spritesheet
    spritesheet
        .save(&png_path)
        .map_err(|e| format!("Failed to save spritesheet: {}", e))?;

    // Create metadata JSON
    let json_path = png_path.replace(".png", ".json");
    let metadata = serde_json::json!({
        "sprite_sheet": PathBuf::from(&png_path).file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| png_path.clone()),
        "animation": animation.name,
        "frame_width": canvas_w,
        "frame_height": canvas_h,
        "columns": cols,
        "rows": rows,
        "frames": frame_metadata
    });

    let json_str = serde_json::to_string_pretty(&metadata)
        .map_err(|e| format!("Failed to serialize metadata: {}", e))?;
    fs::write(&json_path, json_str).map_err(|e| format!("Failed to save metadata: {}", e))?;

    Ok((png_path, json_path))
}

/// Export all animations for the current character
pub fn export_all_animations(state: &AppState, output_dir: &str) -> Result<usize, String> {
    let project = state.project.as_ref().ok_or("No project loaded")?;
    let char_name = state
        .active_character
        .as_ref()
        .ok_or("No character selected")?;
    let character = project.get_character(char_name).ok_or("Character not found")?;

    // Create output directory if needed
    fs::create_dir_all(output_dir)
        .map_err(|e| format!("Failed to create output directory: {}", e))?;

    let mut exported_count = 0;
    let (canvas_w, canvas_h) = character.canvas_size;
    let canvas_size = character.canvas_size;

    for animation in &character.animations {
        if animation.frames.is_empty() {
            continue;
        }

        let frame_count = animation.frames.len();

        // Calculate spritesheet dimensions
        let (cols, rows) = if frame_count <= 8 {
            (frame_count, 1)
        } else {
            let cols = (frame_count as f32).sqrt().ceil() as usize;
            let rows = (frame_count + cols - 1) / cols;
            (cols, rows)
        };

        let sheet_w = cols as u32 * canvas_w;
        let sheet_h = rows as u32 * canvas_h;
        let mut spritesheet = image::RgbaImage::new(sheet_w, sheet_h);

        // Render each frame
        let mut frame_metadata = Vec::new();
        for (i, frame) in animation.frames.iter().enumerate() {
            let frame_img = render_frame_to_image(project, animation, i, canvas_size)?;

            let col = i % cols;
            let row = i / cols;
            let x = col as u32 * canvas_w;
            let y = row as u32 * canvas_h;

            for (px, py, pixel) in frame_img.enumerate_pixels() {
                spritesheet.put_pixel(x + px, y + py, *pixel);
            }

            frame_metadata.push(serde_json::json!({
                "x": x,
                "y": y,
                "width": canvas_w,
                "height": canvas_h,
                "duration_ms": frame.duration_ms
            }));
        }

        // Sanitize animation name for filename
        let safe_name: String = animation
            .name
            .chars()
            .map(|c| {
                if c.is_alphanumeric() || c == '_' || c == '-' {
                    c
                } else {
                    '_'
                }
            })
            .collect();
        let png_path = format!("{}/{}_{}.png", output_dir, char_name, safe_name);

        // Save spritesheet
        spritesheet
            .save(&png_path)
            .map_err(|e| format!("Failed to save {}: {}", png_path, e))?;

        // Create metadata JSON
        let json_path = png_path.replace(".png", ".json");
        let metadata = serde_json::json!({
            "sprite_sheet": PathBuf::from(&png_path).file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| png_path.clone()),
            "character": char_name,
            "animation": animation.name,
            "frame_width": canvas_w,
            "frame_height": canvas_h,
            "columns": cols,
            "rows": rows,
            "frames": frame_metadata
        });

        let json_str = serde_json::to_string_pretty(&metadata)
            .map_err(|e| format!("Failed to serialize metadata: {}", e))?;
        fs::write(&json_path, json_str)
            .map_err(|e| format!("Failed to save {}: {}", json_path, e))?;

        exported_count += 1;
    }

    Ok(exported_count)
}
