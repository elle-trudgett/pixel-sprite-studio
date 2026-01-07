use bevy_egui::egui;

use crate::state::ZOOM_LEVELS;

/// Format zoom level: integers show as "16x", fractional as "0.25x"
pub fn format_zoom(level: f32) -> String {
    if level.fract() == 0.0 {
        format!("{}x", level as i32)
    } else {
        format!("{}x", level)
    }
}

/// Calculate the best zoom level to fit the canvas in the available area
pub fn calculate_fit_zoom(canvas_size: (u32, u32), available: egui::Vec2, ppp: f32) -> f32 {
    // Find the largest zoom level where the canvas fits in the available area
    // Account for some padding (90% of available space)
    let padded_w = available.x * 0.9;
    let padded_h = available.y * 0.9;

    // Calculate max zoom for each dimension
    let max_zoom_w = (padded_w * ppp) / canvas_size.0 as f32;
    let max_zoom_h = (padded_h * ppp) / canvas_size.1 as f32;
    let max_zoom = max_zoom_w.min(max_zoom_h);

    // Find the largest ZOOM_LEVEL that's <= max_zoom
    ZOOM_LEVELS
        .iter()
        .rev()
        .find(|&&level| level <= max_zoom)
        .copied()
        .unwrap_or(ZOOM_LEVELS[0])
}

/// Get a scaled font size with minimum of 12
pub fn scaled_font(base_size: f32, scale: f32) -> f32 {
    (base_size.max(12.0) * scale).max(12.0)
}

/// Get a scaled margin/spacing value
pub fn scaled_margin(base_size: f32, scale: f32) -> f32 {
    base_size * scale
}

/// Format a duration as a human-readable relative time string
pub fn format_relative_time(elapsed: std::time::Duration) -> String {
    let secs = elapsed.as_secs();

    if secs < 3 {
        "just now".to_string()
    } else if secs < 60 {
        format!("{} seconds ago", secs)
    } else if secs < 120 {
        "1 minute ago".to_string()
    } else if secs < 3600 {
        format!("{} minutes ago", secs / 60)
    } else if secs < 7200 {
        "1 hour ago".to_string()
    } else if secs < 86400 {
        format!("{} hours ago", secs / 3600)
    } else if secs < 172800 {
        "1 day ago".to_string()
    } else {
        format!("{} days ago", secs / 86400)
    }
}

/// Render a tab-style button that looks distinct from regular selectable labels
pub fn tab_button(
    ui: &mut egui::Ui,
    selected: bool,
    text: impl Into<String>,
    ui_scale: f32,
) -> egui::Response {
    let text = text.into();
    let padding = egui::vec2(scaled_margin(8.0, ui_scale), scaled_margin(4.0, ui_scale));

    let text_color = if selected {
        egui::Color32::WHITE
    } else {
        egui::Color32::from_gray(180)
    };

    let bg_color = if selected {
        egui::Color32::from_rgb(70, 90, 120)
    } else {
        egui::Color32::from_gray(50)
    };

    let galley = ui.painter().layout_no_wrap(
        text.clone(),
        egui::FontId::proportional(scaled_font(14.0, ui_scale)),
        text_color,
    );

    let tab_size = galley.size() + padding * 2.0;
    // Selected: full height, lifted 1px to show stroke
    // Deselected: 2px lower (partially hidden below separator)
    let deselected_sink = 2.0;
    let lift = 1.0;
    let desired_size = egui::vec2(tab_size.x, tab_size.y + lift);
    let (rect, response) = ui.allocate_exact_size(desired_size, egui::Sense::click());

    if ui.is_rect_visible(rect) {
        let y_offset = if selected { 0.0 } else { deselected_sink };
        let draw_rect = egui::Rect::from_min_size(rect.min + egui::vec2(0.0, y_offset), tab_size);

        let bg = if response.hovered() && !selected {
            egui::Color32::from_rgb(55, 65, 80)
        } else {
            bg_color
        };

        // Draw background with rounded top corners only
        ui.painter().rect_filled(
            draw_rect,
            egui::Rounding {
                nw: 4.0,
                ne: 4.0,
                sw: 0.0,
                se: 0.0,
            },
            bg,
        );

        // Draw bottom border - bright for selected, dark gray for deselected
        // Draw 1px up so the 2px stroke is fully inside the tab
        let stroke_y = draw_rect.max.y - 1.0;
        let stroke_color = if selected {
            egui::Color32::from_rgb(100, 140, 200)
        } else {
            egui::Color32::from_gray(40)
        };
        ui.painter().line_segment(
            [
                egui::pos2(draw_rect.min.x, stroke_y),
                egui::pos2(draw_rect.max.x, stroke_y),
            ],
            egui::Stroke::new(2.0, stroke_color),
        );

        // Draw text centered
        ui.painter().galley(draw_rect.min + padding, galley, text_color);
    }

    response
}
