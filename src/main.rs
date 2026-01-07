mod export;
mod file;
mod imaging;
mod model;
mod state;
mod ui;

use bevy::prelude::*;
use bevy::window::{PrimaryWindow, WindowCloseRequested};
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
        .add_systems(Startup, (setup, configure_fonts))
        .add_systems(Update, (ui_system, handle_window_close, update_window_title))
        .run();
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
