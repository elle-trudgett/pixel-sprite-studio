use bevy_egui::egui;
use std::collections::HashMap;

use crate::imaging::decode_base64_to_texture;
use crate::state::AppState;
use crate::ui::widgets::scaled_font;

/// Renders a circular rotation wheel for importing/viewing rotation sprites
pub fn render_rotation_wheel(
    ui: &mut egui::Ui,
    state: &mut AppState,
    char_name: &str,
    rotations: &[(u16, bool)],
) {
    // Push a unique ID scope for this wheel instance
    let part_name = state.editor_selected_part.as_deref().unwrap_or("none");
    let state_name = state.editor_selected_state.as_deref().unwrap_or("default");
    ui.push_id(format!("rot_wheel_{}_{}", part_name, state_name), |ui| {
        let available = ui.available_size();
        let wheel_size = available.x.min(500.0);
        let center_y = 250.0; // Fixed height for the wheel area
        let radius = 120.0; // Increased radius
        let slot_size = 64.0; // Larger slots for sprites

        // Reserve space for the wheel
        let (response, painter) = ui.allocate_painter(
            egui::vec2(wheel_size, center_y * 2.0),
            egui::Sense::hover(),
        );

        let center = response.rect.center();

        // Get the angles - default to 8 rotations (45 degree mode)
        let angles: Vec<u16> = if rotations.is_empty() {
            vec![0, 45, 90, 135, 180, 225, 270, 315]
        } else {
            let mut sorted: Vec<u16> = rotations.iter().map(|(a, _)| *a).collect();
            sorted.sort();
            sorted
        };

        // Create a map of angle -> has_image for quick lookup
        let rotation_map: HashMap<u16, bool> = rotations.iter().cloned().collect();

        // Draw slots in a circle
        for angle in &angles {
            // Convert angle to radians - 0° = East (right), counterclockwise
            // 0° = E, 90° = N, 180° = W, 270° = S
            // In screen coordinates, Y increases downward, so negate sin
            let rad = (*angle as f32).to_radians();
            let slot_center = center + egui::vec2(rad.cos() * radius, -rad.sin() * radius);

            // Slot rectangle
            let slot_rect = egui::Rect::from_center_size(slot_center, egui::vec2(slot_size, slot_size));

            let has_image = rotation_map.get(angle).copied().unwrap_or(false);

            // Draw slot background
            let bg_color = if has_image {
                egui::Color32::from_rgb(40, 40, 40) // Dark bg for images
            } else {
                egui::Color32::from_rgb(60, 60, 60) // Gray for empty
            };
            painter.rect_filled(slot_rect, 4.0, bg_color);

            // Try to draw the sprite image if it exists
            if has_image {
                let texture_key = format!("{}/{}/{}/{}", char_name, part_name, state_name, angle);

                // Get or create texture
                if !state.texture_cache.contains_key(&texture_key) {
                    // Try to load the image data
                    if let Some(ref project) = state.project {
                        if let Some(character) = project.get_character(char_name) {
                            if let Some(part) = character.get_part(part_name) {
                                if let Some(state_obj) = part.states.iter().find(|s| s.name == state_name) {
                                    if let Some(rotation) = state_obj.rotations.get(angle) {
                                        if let Some(ref base64_data) = rotation.image_data {
                                            if let Ok(texture) =
                                                decode_base64_to_texture(ui.ctx(), &texture_key, base64_data)
                                            {
                                                state.texture_cache.insert(texture_key.clone(), texture);
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }

                // Draw the texture if we have it
                if let Some(texture) = state.texture_cache.get(&texture_key) {
                    let uv = egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0));
                    painter.image(texture.id(), slot_rect.shrink(2.0), uv, egui::Color32::WHITE);
                }
            }

            painter.rect_stroke(slot_rect, 4.0, egui::Stroke::new(1.0, egui::Color32::WHITE));

            // Draw angle label below the slot
            let label_pos = slot_center + egui::vec2(0.0, slot_size / 2.0 + 10.0);
            painter.text(
                label_pos,
                egui::Align2::CENTER_CENTER,
                format!("{}°", angle),
                egui::FontId::proportional(scaled_font(12.0, state.config.ui_scale)),
                egui::Color32::WHITE,
            );

            // Check for click on this slot
            let slot_response =
                ui.interact(slot_rect, ui.id().with(("rot_slot", *angle)), egui::Sense::click());
            if slot_response.clicked() {
                // Store angle for pending import (file picker will be called after render)
                state.pending_rotation_import = Some(*angle);
            }

            if slot_response.hovered() {
                painter.rect_stroke(slot_rect, 4.0, egui::Stroke::new(2.0, egui::Color32::YELLOW));
            }
        }

        // Draw center label
        let part_name = state.editor_selected_part.as_deref().unwrap_or("?");
        let state_name = state.editor_selected_state.as_deref().unwrap_or("default");
        painter.text(
            center,
            egui::Align2::CENTER_CENTER,
            format!("{}\n{}", part_name, state_name),
            egui::FontId::proportional(scaled_font(12.0, state.config.ui_scale)),
            egui::Color32::WHITE,
        );

        // Draw compass labels (0° = East, counterclockwise)
        let compass_radius = radius + slot_size / 2.0 + 25.0;
        let compass_labels = [
            (0.0_f32, "E (0°)"),
            (90.0_f32, "N (90°)"),
            (180.0_f32, "W (180°)"),
            (270.0_f32, "S (270°)"),
        ];
        for (deg, label) in compass_labels {
            let rad = deg.to_radians();
            // Same formula: x = cos, y = -sin for counterclockwise from East
            let pos = center + egui::vec2(rad.cos() * compass_radius, -rad.sin() * compass_radius);
            painter.text(
                pos,
                egui::Align2::CENTER_CENTER,
                label,
                egui::FontId::proportional(scaled_font(12.0, state.config.ui_scale)),
                egui::Color32::GRAY,
            );
        }
    }); // close push_id
}
