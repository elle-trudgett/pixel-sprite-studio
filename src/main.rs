mod export;
mod file;
mod imaging;
mod model;
mod state;
mod ui;

use bevy::prelude::*;
use bevy::window::{Monitor, PrimaryWindow, WindowCloseRequested};
use bevy_egui::{egui, EguiContexts, EguiPlugin};

use state::PendingAction;
use state::AppState;
use ui::ui_system;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "Pixel Sprite Studio".to_string(),
                resolution: (1280.0, 800.0).into(),
                ..default()
            }),
            ..default()
        }))
        .add_plugins(EguiPlugin)
        .init_resource::<AppState>()
        .init_resource::<WindowSizeAdjusted>()
        .add_systems(Startup, (setup, configure_fonts))
        .add_systems(Update, (ui_system, handle_window_close, update_window_title, adjust_window_size))
        .run();
}

/// Tracks whether we've already adjusted window size (runs once)
#[derive(Resource, Default)]
struct WindowSizeAdjusted(bool);

/// Adjust window size based on monitor resolution (runs once on first frame)
fn adjust_window_size(
    mut adjusted: ResMut<WindowSizeAdjusted>,
    monitors: Query<&Monitor>,
    mut windows: Query<&mut Window, With<PrimaryWindow>>,
) {
    if adjusted.0 {
        return;
    }
    adjusted.0 = true;

    // Find the primary or largest monitor
    let monitor_size = monitors
        .iter()
        .max_by_key(|m| {
            let size = m.physical_size();
            size.x * size.y
        })
        .map(|m| m.physical_size());

    if let (Some(size), Ok(mut window)) = (monitor_size, windows.get_single_mut()) {
        // If monitor is at least 125% of 1920x1080 (2400x1350), use larger window
        let min_width = (1920.0 * 1.25) as u32;
        let min_height = (1080.0 * 1.25) as u32;

        if size.x >= min_width && size.y >= min_height {
            window.resolution.set(1920.0, 1080.0);
        }
    }
}

fn setup(mut commands: Commands) {
    commands.spawn(Camera2d);
}

fn configure_fonts(mut contexts: EguiContexts) {
    let ctx = contexts.ctx_mut();
    let mut fonts = egui::FontDefinitions::default();

    // Ensure emoji support - egui should include basic Unicode support
    fonts.families.entry(egui::FontFamily::Proportional).or_default();
    fonts.families.entry(egui::FontFamily::Monospace).or_default();

    ctx.set_fonts(fonts);
}

fn handle_window_close(
    mut close_events: EventReader<WindowCloseRequested>,
    mut state: ResMut<AppState>,
) {
    for _event in close_events.read() {
        if state.has_unsaved_changes() {
            state.pending_action = Some(PendingAction::Exit);
        } else {
            std::process::exit(0);
        }
    }
}

fn update_window_title(state: Res<AppState>, mut windows: Query<&mut Window, With<PrimaryWindow>>) {
    if let Ok(mut window) = windows.get_single_mut() {
        let title = match (&state.project, &state.project_path) {
            (Some(project), Some(path)) => {
                let dirty = if state.has_unsaved_changes() { " *" } else { "" };
                let filename = std::path::Path::new(path)
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_else(|| "unknown".to_string());
                format!("{} - {}{} - Pixel Sprite Studio", project.name, filename, dirty)
            }
            (Some(project), None) => {
                let dirty = if state.has_unsaved_changes() { " *" } else { "" };
                format!("{}{} - Pixel Sprite Studio", project.name, dirty)
            }
            _ => "Pixel Sprite Studio".to_string(),
        };
        if window.title != title {
            window.title = title;
        }
    }
}
