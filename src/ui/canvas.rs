use bevy_egui::egui;

use crate::imaging::{
    decode_base64_to_texture, decode_base64_to_yellow_texture, is_pixel_opaque,
    load_reference_texture,
};
use crate::state::ActiveTab;
use crate::state::AppState;
use crate::ui::widgets::{calculate_fit_zoom, scaled_font, scaled_margin};

// Info needed for rendering a placed part
struct PlacedPartRenderInfo {
    id: u64,
    part_name: String,
    layer_name: String,
    character_id: u64,
    character_name: String, // Used for texture cache keys
    state_name: String,
    rotation: u16,
    position: (f32, f32),
    image_data: Option<String>,
    visible: bool,
}

pub fn render_canvas(ui: &mut egui::Ui, state: &mut AppState) {
    // Reference image render info
    struct ReferenceRenderInfo {
        file_path: String,
        position: (f32, f32),
        scale: f32,
        thumbnail: Option<String>,
    }

    // Capture values from project upfront to avoid borrow conflicts
    let (canvas_size, placed_parts, char_name, anim_name, anim_info, reference_info) = {
        let Some(ref project) = state.project else {
            return;
        };
        let active_char = state.active_character.as_ref();

        let char_name = active_char.cloned().unwrap_or_default();
        let (anim_name, anim_info) = active_char
            .and_then(|name| project.get_character(name))
            .and_then(|c| c.animations.get(state.current_animation))
            .map(|a| (a.name.clone(), Some((a.frames.len(), a.fps))))
            .unwrap_or((String::new(), None));

        // Get reference info for current frame
        let reference_info: Option<ReferenceRenderInfo> = active_char
            .and_then(|name| project.get_character(name))
            .and_then(|c| c.animations.get(state.current_animation))
            .and_then(|anim| anim.frames.get(state.current_frame))
            .and_then(|frame| frame.reference.as_ref())
            .map(|r| ReferenceRenderInfo {
                file_path: r.file_path.clone(),
                position: r.position,
                scale: r.scale,
                thumbnail: project.reference_thumbnails.get(&r.file_path).cloned(),
            });

        let parts: Vec<PlacedPartRenderInfo> = active_char
            .and_then(|name| project.get_character(name))
            .and_then(|c| c.animations.get(state.current_animation))
            .and_then(|anim| anim.frames.get(state.current_frame))
            .map(|frame| {
                frame
                    .placed_parts
                    .iter()
                    .map(|p| {
                        // Look up character by ID (stable across renames)
                        let character = project.get_character_by_id(p.character_id);

                        // Look up image data for this part
                        let image_data = character
                            .and_then(|c| c.get_part(&p.part_name))
                            .and_then(|part| part.states.iter().find(|s| s.name == p.state_name))
                            .and_then(|s| s.rotations.get(&p.rotation))
                            .and_then(|r| r.image_data.clone());

                        // Get character name for texture cache keys
                        let character_name = character.map(|c| c.name.clone()).unwrap_or_default();

                        PlacedPartRenderInfo {
                            id: p.id,
                            part_name: p.part_name.clone(),
                            layer_name: if p.layer_name.is_empty() {
                                p.part_name.clone()
                            } else {
                                p.layer_name.clone()
                            },
                            character_id: p.character_id,
                            character_name,
                            state_name: p.state_name.clone(),
                            rotation: p.rotation,
                            position: p.position,
                            image_data,
                            visible: p.visible,
                        }
                    })
                    .collect()
            })
            .unwrap_or_default();

        // Get canvas size from active character
        let canvas_size = active_char
            .and_then(|name| project.get_character(name))
            .map(|c| c.canvas_size)
            .unwrap_or((64, 64));

        (canvas_size, parts, char_name, anim_name, anim_info, reference_info)
    };

    let available = ui.available_size();
    // Account for DPI scaling to get true 1:1 pixel rendering at zoom 1.0
    let ppp = ui.ctx().pixels_per_point();
    let effective_zoom = state.zoom_level / ppp;
    let canvas_w = canvas_size.0 as f32 * effective_zoom;
    let canvas_h = canvas_size.1 as f32 * effective_zoom;

    // Center the canvas with pan offset
    let offset_x = (available.x - canvas_w) / 2.0 + state.canvas_offset.0;
    let offset_y = (available.y - canvas_h) / 2.0 + state.canvas_offset.1;

    let (response, painter) = ui.allocate_painter(available, egui::Sense::click_and_drag());

    // Floor canvas origin in SCREEN PIXEL space, then convert back to points.
    // This ensures we start at an actual pixel boundary.
    let canvas_origin_x = ((response.rect.min.x + offset_x) * ppp).floor() / ppp;
    let canvas_origin_y = ((response.rect.min.y + offset_y) * ppp).floor() / ppp;
    let canvas_origin = egui::pos2(canvas_origin_x, canvas_origin_y);

    // Canvas size should also be calculated in pixels then converted
    // zoom_level = screen pixels per canvas pixel
    let canvas_w_pixels = canvas_size.0 as f32 * state.zoom_level;
    let canvas_h_pixels = canvas_size.1 as f32 * state.zoom_level;
    let canvas_rect = egui::Rect::from_min_size(
        canvas_origin,
        egui::vec2(canvas_w_pixels / ppp, canvas_h_pixels / ppp),
    );

    // Fit/Center button in top-right corner
    let button_size = scaled_margin(24.0, state.config.ui_scale);
    let button_margin = scaled_margin(8.0, state.config.ui_scale);
    let button_rect = egui::Rect::from_min_size(
        egui::pos2(
            response.rect.max.x - button_size - button_margin,
            response.rect.min.y + button_margin,
        ),
        egui::vec2(button_size, button_size),
    );
    let fit_button = ui.put(
        button_rect,
        egui::Button::new(
            egui::RichText::new("⊙").size(scaled_font(16.0, state.config.ui_scale)),
        )
        .min_size(egui::vec2(button_size, button_size)),
    );
    if fit_button
        .on_hover_text("Fit canvas to view and center")
        .clicked()
    {
        state.zoom_level = calculate_fit_zoom(canvas_size, available, ppp);
        state.canvas_offset = (0.0, 0.0);
    }

    // Check for panning input (space key or middle mouse button)
    let space_held = ui.input(|i| i.key_down(egui::Key::Space));
    let middle_mouse_held = ui.input(|i| i.pointer.middle_down());
    let is_panning = space_held || middle_mouse_held;

    // Track whether panning started inside the canvas
    let was_panning = state.is_panning;
    if is_panning && !was_panning {
        // Panning just started - check if mouse is inside canvas area
        if let Some(pos) = ui.input(|i| i.pointer.hover_pos()) {
            state.pan_started_in_canvas = response.rect.contains(pos);
        } else {
            state.pan_started_in_canvas = false;
        }
    } else if !is_panning {
        // Panning ended - reset the flag
        state.pan_started_in_canvas = false;
    }
    state.is_panning = is_panning;

    // Set cursor to grabbing hand when panning (only if started in canvas)
    if is_panning && state.pan_started_in_canvas {
        ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);
    }

    // Draw canvas frame background (the actual sprite canvas)
    painter.rect_filled(canvas_rect, 0.0, egui::Color32::from_rgb(40, 40, 50));

    // Helper to render reference image
    let reference_opacity = state.reference_opacity;
    let reference_show_on_top = state.reference_show_on_top;
    let zoom_level = state.zoom_level;
    let render_reference = |painter: &egui::Painter,
                            ref_info: &ReferenceRenderInfo,
                            texture: &egui::TextureHandle,
                            original_size: (u32, u32),
                            opacity: f32| {
        // Calculate position in screen pixels, then convert to points
        let ref_origin_pixels_x = canvas_origin_x * ppp;
        let ref_origin_pixels_y = canvas_origin_y * ppp;
        let ref_pixels_x = ref_origin_pixels_x + ref_info.position.0 * zoom_level;
        let ref_pixels_y = ref_origin_pixels_y + ref_info.position.1 * zoom_level;
        let screen_x = ref_pixels_x / ppp;
        let screen_y = ref_pixels_y / ppp;

        // Calculate displayed size in screen pixels, then convert to points
        // (original size * scale * zoom = screen pixels)
        let display_w = original_size.0 as f32 * ref_info.scale * zoom_level / ppp;
        let display_h = original_size.1 as f32 * ref_info.scale * zoom_level / ppp;

        let ref_rect = egui::Rect::from_min_size(
            egui::pos2(screen_x, screen_y),
            egui::vec2(display_w, display_h),
        );

        // Draw with opacity
        let alpha = (opacity * 255.0) as u8;
        let tint = egui::Color32::from_rgba_unmultiplied(255, 255, 255, alpha);

        painter.image(
            texture.id(),
            ref_rect,
            egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
            tint,
        );
    };

    // Load reference texture if needed and render behind grid (if not show_on_top)
    if let Some(ref ref_info) = reference_info {
        if !reference_show_on_top {
            // Try to get cached texture or load it
            if !state
                .reference_texture_cache
                .contains_key(&ref_info.file_path)
            {
                if let Ok((texture, original_size, using_fallback)) = load_reference_texture(
                    ui.ctx(),
                    &ref_info.file_path,
                    ref_info.thumbnail.as_deref(),
                ) {
                    state
                        .reference_texture_cache
                        .insert(ref_info.file_path.clone(), (texture, original_size));
                    state
                        .reference_using_fallback
                        .insert(ref_info.file_path.clone(), using_fallback);
                }
            }

            if let Some((texture, original_size)) =
                state.reference_texture_cache.get(&ref_info.file_path)
            {
                render_reference(&painter, ref_info, texture, *original_size, reference_opacity);
            }
        }
    }

    // Draw grid if enabled - calculate in pixel space for consistency with sprites
    if state.show_grid {
        let grid_color = egui::Color32::from_rgba_unmultiplied(100, 100, 100, 60);
        // Canvas origin in screen pixels
        let origin_pixels_x = canvas_origin_x * ppp;
        let origin_pixels_y = canvas_origin_y * ppp;

        for i in 0..=canvas_size.0 {
            // Calculate grid line position in pixels, convert to points
            let x_pixels = origin_pixels_x + i as f32 * state.zoom_level;
            let x = x_pixels / ppp;
            painter.line_segment(
                [
                    egui::pos2(x, canvas_rect.min.y),
                    egui::pos2(x, canvas_rect.max.y),
                ],
                egui::Stroke::new(1.0, grid_color),
            );
        }

        for i in 0..=canvas_size.1 {
            let y_pixels = origin_pixels_y + i as f32 * state.zoom_level;
            let y = y_pixels / ppp;
            painter.line_segment(
                [
                    egui::pos2(canvas_rect.min.x, y),
                    egui::pos2(canvas_rect.max.x, y),
                ],
                egui::Stroke::new(1.0, grid_color),
            );
        }
    }

    // Canvas border (draw before sprites so it appears underneath)
    painter.rect_stroke(
        canvas_rect,
        0.0,
        egui::Stroke::new(2.0, egui::Color32::from_rgb(100, 100, 120)),
    );

    // Draw placed parts (in list order - later items drawn on top)
    let show_labels = state.show_labels;
    // Pre-calculate canvas origin in pixels for sprite positioning
    let origin_pixels_x = canvas_origin_x * ppp;
    let origin_pixels_y = canvas_origin_y * ppp;

    for part_info in &placed_parts {
        // Skip invisible layers
        if !part_info.visible {
            continue;
        }

        // Calculate sprite position in screen pixels, then convert to points
        let sprite_pixels_x = origin_pixels_x + part_info.position.0 * state.zoom_level;
        let sprite_pixels_y = origin_pixels_y + part_info.position.1 * state.zoom_level;
        let screen_x = sprite_pixels_x / ppp;
        let screen_y = sprite_pixels_y / ppp;

        let is_selected = state.selected_part_id == Some(part_info.id);

        // Try to get or create texture for this part
        let texture_key = format!(
            "{}/{}/{}/{}",
            part_info.character_name, part_info.part_name, part_info.state_name, part_info.rotation
        );

        let mut rendered_texture = false;
        let mut image_size = (16.0_f32, 16.0_f32);
        let mut part_rect = egui::Rect::NOTHING;

        if let Some(ref base64_data) = part_info.image_data {
            // Check if texture is already cached
            if !state.texture_cache.contains_key(&texture_key) {
                // Decode and create texture
                if let Ok(texture) = decode_base64_to_texture(ui.ctx(), &texture_key, base64_data) {
                    state.texture_cache.insert(texture_key.clone(), texture);
                }
            }

            if let Some(texture) = state.texture_cache.get(&texture_key) {
                let tex_size = texture.size_vec2();
                image_size = (tex_size.x, tex_size.y);
                // Calculate size in screen pixels (should be integer), then convert to points
                let size_pixels_x = (tex_size.x * state.zoom_level).round();
                let size_pixels_y = (tex_size.y * state.zoom_level).round();
                let scaled_size = egui::vec2(size_pixels_x / ppp, size_pixels_y / ppp);
                part_rect = egui::Rect::from_min_size(egui::pos2(screen_x, screen_y), scaled_size);

                // Draw the texture
                painter.image(
                    texture.id(),
                    part_rect,
                    egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                    egui::Color32::WHITE,
                );

                rendered_texture = true;
            }
        }

        // Fallback: draw colored rectangle if no texture
        if !rendered_texture {
            let part_size_pixels_x = (image_size.0 * state.zoom_level).round();
            let part_size_pixels_y = (image_size.1 * state.zoom_level).round();
            part_rect = egui::Rect::from_min_size(
                egui::pos2(screen_x, screen_y),
                egui::vec2(part_size_pixels_x / ppp, part_size_pixels_y / ppp),
            );

            // Color based on part name hash for variety
            let hash = part_info
                .part_name
                .bytes()
                .fold(0u32, |acc, b| acc.wrapping_add(b as u32));
            let r = ((hash * 17) % 200 + 55) as u8;
            let g = ((hash * 31) % 200 + 55) as u8;
            let b = ((hash * 47) % 200 + 55) as u8;

            let fill_color = egui::Color32::from_rgba_unmultiplied(r, g, b, 180);
            painter.rect_filled(part_rect, 2.0, fill_color);

            // Draw part name in center
            painter.text(
                part_rect.center(),
                egui::Align2::CENTER_CENTER,
                &part_info.layer_name,
                egui::FontId::proportional(scaled_font(12.0, state.config.ui_scale)),
                egui::Color32::WHITE,
            );
        }

        // Draw yellow flash when part is first selected (no continuous pulse)
        if is_selected && rendered_texture {
            let elapsed = state
                .selection_time
                .map(|t| t.elapsed().as_secs_f32())
                .unwrap_or(1.0);

            // Flash decays exponentially over ~0.5s
            let flash_intensity = (-elapsed * 8.0).exp();

            // Only draw if flash is still visible
            if flash_intensity > 0.01 {
                let alpha = (flash_intensity * 255.0) as u8;

                // Get or create yellow silhouette texture
                let yellow_key = format!("{}_yellow", texture_key);
                if !state.texture_cache.contains_key(&yellow_key) {
                    if let Some(ref base64_data) = part_info.image_data {
                        if let Ok(yellow_tex) =
                            decode_base64_to_yellow_texture(ui.ctx(), &yellow_key, base64_data)
                        {
                            state.texture_cache.insert(yellow_key.clone(), yellow_tex);
                        }
                    }
                }

                // Draw yellow silhouette
                if let Some(yellow_texture) = state.texture_cache.get(&yellow_key) {
                    let uv =
                        egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0));
                    let tint = egui::Color32::from_rgba_unmultiplied(255, 255, 255, alpha);
                    painter.image(yellow_texture.id(), part_rect, uv, tint);
                }

                // Request repaint only while flash is animating
                ui.ctx().request_repaint();
            }
        }

        // Draw label box in top-left corner if show_labels is enabled
        if show_labels {
            let label_text = &part_info.layer_name;
            let font = egui::FontId::proportional(scaled_font(12.0, state.config.ui_scale));
            let small_font = egui::FontId::proportional(scaled_font(10.0, state.config.ui_scale));
            let text_color = egui::Color32::WHITE;
            let bg_color = egui::Color32::from_rgba_unmultiplied(200, 0, 0, 220);
            let state_bg = egui::Color32::from_rgb(50, 80, 130);
            let rot_bg = egui::Color32::from_rgb(50, 100, 60);

            // Measure text sizes
            let galley = painter.layout_no_wrap(label_text.clone(), font.clone(), text_color);
            let text_size = galley.size();
            let state_galley =
                painter.layout_no_wrap(part_info.state_name.clone(), small_font.clone(), text_color);
            let state_size = state_galley.size();
            let rot_text = format!("{}°", part_info.rotation);
            let rot_galley = painter.layout_no_wrap(rot_text, small_font.clone(), text_color);
            let rot_size = rot_galley.size();
            let padding = 2.0;
            let badge_gap = 2.0;

            let label_height = text_size.y + padding * 2.0;
            let label_rect = egui::Rect::from_min_size(
                egui::pos2(part_rect.min.x, part_rect.min.y - label_height),
                egui::vec2(text_size.x + padding * 2.0, label_height),
            );

            // Draw label background
            painter.rect_filled(label_rect, 0.0, bg_color);

            // Draw label text
            painter.galley(
                egui::pos2(label_rect.min.x + padding, label_rect.min.y + padding),
                galley,
                text_color,
            );

            // Draw state badge (blue, flat rectangle)
            let state_rect = egui::Rect::from_min_size(
                egui::pos2(label_rect.max.x + badge_gap, label_rect.min.y),
                egui::vec2(state_size.x + padding * 2.0, label_height),
            );
            painter.rect_filled(state_rect, 0.0, state_bg);
            painter.galley(
                egui::pos2(
                    state_rect.min.x + padding,
                    state_rect.min.y
                        + padding
                        + (label_height - padding * 2.0 - state_size.y) / 2.0,
                ),
                state_galley,
                text_color,
            );

            // Draw rotation badge (green, flat rectangle)
            let rot_rect = egui::Rect::from_min_size(
                egui::pos2(state_rect.max.x + badge_gap, label_rect.min.y),
                egui::vec2(rot_size.x + padding * 2.0, label_height),
            );
            painter.rect_filled(rot_rect, 0.0, rot_bg);
            painter.galley(
                egui::pos2(
                    rot_rect.min.x + padding,
                    rot_rect.min.y + padding + (label_height - padding * 2.0 - rot_size.y) / 2.0,
                ),
                rot_galley,
                text_color,
            );
        }

        // Draw selection border (pulsating yellow) or label border (red) - on top of labels
        if is_selected {
            let t = ui.ctx().input(|i| i.time);
            let alpha = (0.6 + 0.4 * (6.0 * t).sin()) as f32;
            let yellow =
                egui::Color32::from_rgba_unmultiplied(255, 255, 0, (alpha * 255.0) as u8);
            painter.rect_stroke(part_rect, 0.0, egui::Stroke::new(2.0, yellow));
            ui.ctx().request_repaint(); // Keep animating
        } else if show_labels {
            painter.rect_stroke(part_rect, 0.0, egui::Stroke::new(1.0, egui::Color32::RED));
        }
    }

    // Render reference image on top (if show_on_top)
    if let Some(ref ref_info) = reference_info {
        if reference_show_on_top {
            // Try to get cached texture or load it
            if !state
                .reference_texture_cache
                .contains_key(&ref_info.file_path)
            {
                if let Ok((texture, original_size, using_fallback)) = load_reference_texture(
                    ui.ctx(),
                    &ref_info.file_path,
                    ref_info.thumbnail.as_deref(),
                ) {
                    state
                        .reference_texture_cache
                        .insert(ref_info.file_path.clone(), (texture, original_size));
                    state
                        .reference_using_fallback
                        .insert(ref_info.file_path.clone(), using_fallback);
                }
            }

            if let Some((texture, original_size)) =
                state.reference_texture_cache.get(&ref_info.file_path)
            {
                render_reference(&painter, ref_info, texture, *original_size, reference_opacity);
            }
        }
    }

    // Check for shift key for reference image dragging
    let shift_held = ui.input(|i| i.modifiers.shift);
    let has_reference = reference_info.is_some();

    // Handle Shift+MMB/Space for reference image dragging
    if shift_held && has_reference && (space_held || middle_mouse_held) {
        let delta = if space_held {
            ui.input(|i| i.pointer.delta())
        } else {
            response.drag_delta()
        };

        // Convert screen delta to canvas coordinates
        let canvas_delta_x = delta.x / effective_zoom;
        let canvas_delta_y = delta.y / effective_zoom;

        // Update reference position in project
        if let Some(ref mut project) = state.project {
            if let Some(ref cn) = state.active_character {
                if let Some(character) = project.get_character_mut(cn) {
                    if let Some(anim) = character.animations.get_mut(state.current_animation) {
                        if let Some(frame) = anim.frames.get_mut(state.current_frame) {
                            if let Some(ref mut frame_ref) = frame.reference {
                                frame_ref.position.0 += canvas_delta_x;
                                frame_ref.position.1 += canvas_delta_y;
                            }
                        }
                    }
                }
            }
        }

        // Set cursor to indicate dragging
        ui.ctx().set_cursor_icon(egui::CursorIcon::Move);
    } else if space_held && state.pan_started_in_canvas {
        // Space: pan by just moving the mouse (no click needed)
        let delta = ui.input(|i| i.pointer.delta());
        state.canvas_offset.0 += delta.x;
        state.canvas_offset.1 += delta.y;
    } else if middle_mouse_held && state.pan_started_in_canvas && response.dragged() {
        // Middle mouse: pan while dragging
        let delta = response.drag_delta();
        state.canvas_offset.0 += delta.x;
        state.canvas_offset.1 += delta.y;
    }

    // Clamp canvas offset so canvas center stays within viewport
    let max_offset_x = available.x * 0.5;
    let max_offset_y = available.y * 0.5;
    state.canvas_offset.0 = state
        .canvas_offset
        .0
        .clamp(-max_offset_x, max_offset_x);
    state.canvas_offset.1 = state
        .canvas_offset
        .1
        .clamp(-max_offset_y, max_offset_y);

    // Handle gallery drag drop onto canvas
    let mouse_released = ui.input(|i| i.pointer.any_released());
    if mouse_released && state.gallery_drag.is_some() {
        if let Some(pos) = ui.input(|i| i.pointer.interact_pos()) {
            // Check if dropped within canvas bounds
            if canvas_rect.contains(pos) {
                // Convert screen position to canvas coordinates
                let canvas_x = (pos.x - canvas_rect.min.x) / effective_zoom;
                let canvas_y = (pos.y - canvas_rect.min.y) / effective_zoom;

                // Place the part (centered on drop position)
                if let Some(gallery_drag) = state.gallery_drag.take() {
                    // Get sprite size to center it
                    let texture_key = format!(
                        "gallery/{}/{}",
                        gallery_drag.character_name, gallery_drag.part_name
                    );
                    let sprite_size = state
                        .texture_cache
                        .get(&texture_key)
                        .map(|t| t.size_vec2())
                        .unwrap_or(egui::vec2(16.0, 16.0));

                    // Offset by half sprite size to center on cursor
                    let centered_x = canvas_x - sprite_size.x / 2.0;
                    let centered_y = canvas_y - sprite_size.y / 2.0;

                    let pixel_aligned = state.pixel_aligned;
                    let (x, y) = if pixel_aligned {
                        (centered_x.round(), centered_y.round())
                    } else {
                        (centered_x, centered_y)
                    };
                    state.place_part_on_canvas(
                        gallery_drag.character_id,
                        &gallery_drag.part_name,
                        &gallery_drag.state_name,
                        x,
                        y,
                    );
                    state.set_status(format!(
                        "Placed {} at ({:.0}, {:.0})",
                        gallery_drag.part_name, x, y
                    ));
                }
            }
        }
        state.gallery_drag = None;
    }

    // Change cursor when hovering over draggable parts
    if !is_panning && response.hovered() {
        if let Some(pos) = response.hover_pos() {
            let mut hovering_part = false;
            for part_info in placed_parts.iter().rev() {
                // Use pixel-space calculations to match rendering
                let hit_origin_pixels_x = canvas_origin_x * ppp;
                let hit_origin_pixels_y = canvas_origin_y * ppp;
                let screen_x = (hit_origin_pixels_x + part_info.position.0 * state.zoom_level) / ppp;
                let screen_y = (hit_origin_pixels_y + part_info.position.1 * state.zoom_level) / ppp;
                let part_size = if let Some(texture) = state.texture_cache.get(&format!(
                    "{}/{}/{}/{}",
                    part_info.character_name,
                    part_info.part_name,
                    part_info.state_name,
                    part_info.rotation
                )) {
                    texture.size_vec2() * state.zoom_level / ppp
                } else {
                    egui::vec2(16.0, 16.0) * state.zoom_level / ppp
                };
                let part_rect = egui::Rect::from_min_size(egui::pos2(screen_x, screen_y), part_size);
                if part_rect.contains(pos) {
                    hovering_part = true;
                    break;
                }
            }
            if hovering_part {
                if response.dragged() {
                    ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);
                } else {
                    ui.ctx().set_cursor_icon(egui::CursorIcon::Grab);
                }
            }
        }
    }

    // Handle mouse interactions - select on mousedown (not mouseup)
    let prev_clicked_part = state.last_clicked_part_id;
    let should_check_selection = !is_panning && ui.input(|i| i.pointer.any_pressed());
    if should_check_selection {
        if let Some(pos) = response.interact_pointer_pos() {
            // Collect all parts whose bounding boxes contain the click (top to bottom)
            let mut candidates: Vec<(u64, Option<&String>, f32, f32, egui::Vec2)> = Vec::new();

            for part_info in placed_parts.iter().rev() {
                // Skip invisible layers - they shouldn't be selectable
                if !part_info.visible {
                    continue;
                }
                // Use pixel-space calculations to match rendering
                let click_origin_pixels_x = canvas_origin_x * ppp;
                let click_origin_pixels_y = canvas_origin_y * ppp;
                let screen_x =
                    (click_origin_pixels_x + part_info.position.0 * state.zoom_level) / ppp;
                let screen_y =
                    (click_origin_pixels_y + part_info.position.1 * state.zoom_level) / ppp;
                // Use cached texture size if available, otherwise default 16x16
                let part_size = if let Some(texture) = state.texture_cache.get(&format!(
                    "{}/{}/{}/{}",
                    part_info.character_name,
                    part_info.part_name,
                    part_info.state_name,
                    part_info.rotation
                )) {
                    texture.size_vec2() * state.zoom_level / ppp
                } else {
                    egui::vec2(16.0, 16.0) * state.zoom_level / ppp
                };
                let part_rect = egui::Rect::from_min_size(egui::pos2(screen_x, screen_y), part_size);
                if part_rect.contains(pos) {
                    candidates.push((
                        part_info.id,
                        part_info.image_data.as_ref(),
                        screen_x,
                        screen_y,
                        part_size,
                    ));
                }
            }

            // Find the topmost part with a non-transparent pixel at click location
            let mut clicked_part = None;
            let mut topmost_fallback = None;

            for (id, image_data, screen_x, screen_y, _part_size) in &candidates {
                // Remember the topmost as fallback
                if topmost_fallback.is_none() {
                    topmost_fallback = Some(*id);
                }

                // Calculate pixel coordinates within the part
                let pixel_x = ((pos.x - screen_x) * ppp / state.zoom_level) as u32;
                let pixel_y = ((pos.y - screen_y) * ppp / state.zoom_level) as u32;

                // Check if pixel is opaque
                if let Some(data) = image_data {
                    if is_pixel_opaque(data, pixel_x, pixel_y) {
                        clicked_part = Some(*id);
                        break;
                    }
                } else {
                    // No image data means we can't check transparency, treat as opaque
                    clicked_part = Some(*id);
                    break;
                }
            }

            // Use the first opaque hit, or fall back to topmost if all transparent
            let new_selection = clicked_part.or(topmost_fallback);

            // Track what was clicked for double-click validation
            state.last_clicked_part_id = new_selection;

            if new_selection != state.selected_part_id {
                state.selected_part_id = new_selection;
                if new_selection.is_some() {
                    state.selection_time = Some(std::time::Instant::now());
                }
            }

            // Initialize drag accumulator if we selected a part
            if let Some(part) = state.get_selected_placed_part() {
                state.drag_accumulator = part.position;
            }
        }
    }

    // Handle double-click to navigate to character editor for that part
    if !is_panning && response.double_clicked() {
        if let Some(pos) = response.interact_pointer_pos() {
            // Find which part was double-clicked (same logic as single-click)
            for part_info in placed_parts.iter().rev() {
                // Use pixel-space calculations to match rendering
                let dbl_origin_pixels_x = canvas_origin_x * ppp;
                let dbl_origin_pixels_y = canvas_origin_y * ppp;
                let screen_x =
                    (dbl_origin_pixels_x + part_info.position.0 * state.zoom_level) / ppp;
                let screen_y =
                    (dbl_origin_pixels_y + part_info.position.1 * state.zoom_level) / ppp;
                let part_size = if let Some(texture) = state.texture_cache.get(&format!(
                    "{}/{}/{}/{}",
                    part_info.character_name,
                    part_info.part_name,
                    part_info.state_name,
                    part_info.rotation
                )) {
                    texture.size_vec2() * state.zoom_level / ppp
                } else {
                    egui::vec2(16.0, 16.0) * state.zoom_level / ppp
                };
                let part_rect = egui::Rect::from_min_size(egui::pos2(screen_x, screen_y), part_size);

                if part_rect.contains(pos) {
                    // Check pixel transparency if we have image data
                    let pixel_x = ((pos.x - screen_x) * ppp / state.zoom_level) as u32;
                    let pixel_y = ((pos.y - screen_y) * ppp / state.zoom_level) as u32;

                    let is_hit = if let Some(data) = &part_info.image_data {
                        is_pixel_opaque(data, pixel_x, pixel_y)
                    } else {
                        true
                    };

                    if is_hit {
                        // Only navigate if BOTH clicks of the double-click were on the same part
                        if prev_clicked_part == Some(part_info.id) {
                            state.active_tab =
                                ActiveTab::CharacterEditor(part_info.character_name.clone());
                            state.editor_selected_part = Some(part_info.part_name.clone());
                            state.editor_selected_state = Some(part_info.state_name.clone());
                        }
                        break;
                    }
                }
            }
        }
    }

    if !is_panning && response.dragged() && state.selected_part_id.is_some() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);
        let delta = response.drag_delta();
        let zoom = effective_zoom;
        let pixel_aligned = state.pixel_aligned;

        // Accumulate the true position
        state.drag_accumulator.0 += delta.x / zoom;
        state.drag_accumulator.1 += delta.y / zoom;

        // Capture values before mutable borrow
        let new_pos = if pixel_aligned {
            (
                state.drag_accumulator.0.round(),
                state.drag_accumulator.1.round(),
            )
        } else {
            state.drag_accumulator
        };

        // Set the displayed position
        if let Some(part) = state.get_selected_placed_part_mut() {
            part.position = new_pos;
        }
    }

    // Handle mouse wheel for zooming (zoom towards cursor)
    if response.hovered() {
        let scroll_delta = ui.input(|i| i.raw_scroll_delta.y);
        if scroll_delta != 0.0 {
            if let Some(mouse_pos) = ui.input(|i| i.pointer.hover_pos()) {
                // Calculate canvas coordinate under mouse before zoom
                let old_effective_zoom = effective_zoom;
                let canvas_coord_x = (mouse_pos.x - canvas_rect.min.x) / old_effective_zoom;
                let canvas_coord_y = (mouse_pos.y - canvas_rect.min.y) / old_effective_zoom;

                // Apply zoom
                let old_zoom_level = state.zoom_level;
                if scroll_delta > 0.0 {
                    state.zoom_in();
                } else {
                    state.zoom_out();
                }

                // Adjust canvas offset to keep the same canvas coordinate under the mouse
                if state.zoom_level != old_zoom_level {
                    let new_effective_zoom = state.zoom_level / ppp;
                    let new_canvas_w = canvas_size.0 as f32 * new_effective_zoom;
                    let new_canvas_h = canvas_size.1 as f32 * new_effective_zoom;

                    // Where the mouse is relative to the response rect
                    let mouse_rel_x = mouse_pos.x - response.rect.min.x;
                    let mouse_rel_y = mouse_pos.y - response.rect.min.y;

                    // New offset to keep canvas_coord under mouse
                    state.canvas_offset.0 = mouse_rel_x
                        - canvas_coord_x * new_effective_zoom
                        - (available.x - new_canvas_w) / 2.0;
                    state.canvas_offset.1 = mouse_rel_y
                        - canvas_coord_y * new_effective_zoom
                        - (available.y - new_canvas_h) / 2.0;

                    // Re-apply clamping
                    let max_offset_x = available.x * 0.5;
                    let max_offset_y = available.y * 0.5;
                    state.canvas_offset.0 = state
                        .canvas_offset
                        .0
                        .clamp(-max_offset_x, max_offset_x);
                    state.canvas_offset.1 = state
                        .canvas_offset
                        .1
                        .clamp(-max_offset_y, max_offset_y);
                }
            }
        }
    }

    // Overlay info (character name, animation name, canvas info)
    if state.show_overlay_info {
        // Character and animation name at top left corner
        let ui_scale = state.config.ui_scale;
        if !char_name.is_empty() {
            painter.text(
                response.rect.min
                    + egui::vec2(scaled_margin(10.0, ui_scale), scaled_margin(10.0, ui_scale)),
                egui::Align2::LEFT_TOP,
                &char_name,
                egui::FontId::proportional(scaled_font(24.0, ui_scale)),
                egui::Color32::WHITE,
            );
            painter.text(
                response.rect.min
                    + egui::vec2(scaled_margin(10.0, ui_scale), scaled_margin(40.0, ui_scale)),
                egui::Align2::LEFT_TOP,
                &anim_name,
                egui::FontId::proportional(scaled_font(18.0, ui_scale)),
                egui::Color32::GRAY,
            );
            // Frame count, FPS, and duration
            if let Some((frame_count, fps)) = anim_info {
                let duration = frame_count as f32 / fps as f32;
                painter.text(
                    response.rect.min
                        + egui::vec2(scaled_margin(10.0, ui_scale), scaled_margin(64.0, ui_scale)),
                    egui::Align2::LEFT_TOP,
                    format!("{} frames @ {}fps = {:.2}s", frame_count, fps, duration),
                    egui::FontId::proportional(scaled_font(14.0, ui_scale)),
                    egui::Color32::from_gray(140),
                );
            }
        }

        // Canvas info at bottom left corner
        let parts_count = placed_parts.len();
        painter.text(
            egui::pos2(
                response.rect.min.x + scaled_margin(10.0, ui_scale),
                response.rect.max.y - scaled_margin(10.0, ui_scale),
            ),
            egui::Align2::LEFT_BOTTOM,
            format!(
                "{}x{} @ {:.1}x | {} parts",
                canvas_size.0, canvas_size.1, state.zoom_level, parts_count
            ),
            egui::FontId::proportional(scaled_font(12.0, ui_scale)),
            egui::Color32::GRAY,
        );
    }

    // Draw drag indicator when dragging from gallery
    if let Some(ref gallery_drag) = state.gallery_drag {
        if let Some(pos) = ui.input(|i| i.pointer.interact_pos()) {
            let drag_size = 48.0;
            let drag_rect = egui::Rect::from_center_size(pos, egui::vec2(drag_size, drag_size));

            // Draw semi-transparent background
            painter.rect_filled(
                drag_rect,
                4.0,
                egui::Color32::from_rgba_unmultiplied(60, 60, 80, 200),
            );

            // Try to draw the thumbnail
            let texture_key = format!(
                "gallery/{}/{}",
                gallery_drag.character_name, gallery_drag.part_name
            );
            if let Some(texture) = state.texture_cache.get(&texture_key) {
                let uv = egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0));
                painter.image(texture.id(), drag_rect.shrink(2.0), uv, egui::Color32::WHITE);
            } else {
                // Fallback: show part name
                painter.text(
                    drag_rect.center(),
                    egui::Align2::CENTER_CENTER,
                    &gallery_drag.part_name,
                    egui::FontId::proportional(scaled_font(12.0, state.config.ui_scale)),
                    egui::Color32::WHITE,
                );
            }

            painter.rect_stroke(
                drag_rect,
                4.0,
                egui::Stroke::new(2.0, egui::Color32::YELLOW),
            );

            // Show "drop here" indicator if over canvas
            if canvas_rect.contains(pos) {
                let canvas_x = (pos.x - canvas_rect.min.x) / effective_zoom;
                let canvas_y = (pos.y - canvas_rect.min.y) / effective_zoom;
                let label = format!("({:.0}, {:.0})", canvas_x, canvas_y);
                painter.text(
                    pos + egui::vec2(drag_size / 2.0 + 5.0, 0.0),
                    egui::Align2::LEFT_CENTER,
                    label,
                    egui::FontId::proportional(scaled_font(12.0, state.config.ui_scale)),
                    egui::Color32::YELLOW,
                );
            }
        }
    }
}
