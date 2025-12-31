use bevy::prelude::*;
use bevy_egui::{egui, EguiContexts, EguiPlugin};

fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "Sprite Animator".into(),
                resolution: (1280., 720.).into(),
                ..default()
            }),
            ..default()
        }))
        .add_plugins(EguiPlugin)
        .add_systems(Startup, setup)
        .add_systems(Update, ui_system)
        .run();
}

fn setup(mut commands: Commands) {
    commands.spawn(Camera2d);
}

fn ui_system(mut contexts: EguiContexts) {
    egui::TopBottomPanel::top("menu_bar").show(contexts.ctx_mut(), |ui| {
        egui::menu::bar(ui, |ui| {
            ui.menu_button("File", |ui| {
                if ui.button("New Project").clicked() {
                    // TODO: Implement new project
                }
                if ui.button("Open...").clicked() {
                    // TODO: Implement open
                }
                if ui.button("Save").clicked() {
                    // TODO: Implement save
                }
                ui.separator();
                if ui.button("Exit").clicked() {
                    std::process::exit(0);
                }
            });
            ui.menu_button("Edit", |ui| {
                if ui.button("Undo").clicked() {
                    // TODO: Implement undo
                }
                if ui.button("Redo").clicked() {
                    // TODO: Implement redo
                }
            });
            ui.menu_button("View", |ui| {
                if ui.button("Toggle Grid").clicked() {
                    // TODO: Implement grid toggle
                }
                if ui.button("Toggle Reference Layer").clicked() {
                    // TODO: Implement reference toggle
                }
            });
            ui.menu_button("Animation", |ui| {
                if ui.button("Play").clicked() {
                    // TODO: Implement play
                }
                if ui.button("Pause").clicked() {
                    // TODO: Implement pause
                }
                if ui.button("Stop").clicked() {
                    // TODO: Implement stop
                }
            });
            ui.menu_button("Export", |ui| {
                if ui.button("Export Current Animation...").clicked() {
                    // TODO: Implement export single
                }
                if ui.button("Export All Animations...").clicked() {
                    // TODO: Implement export all
                }
            });
        });
    });

    egui::SidePanel::left("asset_browser")
        .default_width(200.0)
        .show(contexts.ctx_mut(), |ui| {
            ui.heading("Asset Browser");
            ui.separator();
            ui.label("No project loaded");
        });

    egui::SidePanel::right("inspector")
        .default_width(200.0)
        .show(contexts.ctx_mut(), |ui| {
            ui.heading("Inspector");
            ui.separator();
            ui.label("No selection");
        });

    egui::TopBottomPanel::bottom("timeline")
        .default_height(150.0)
        .show(contexts.ctx_mut(), |ui| {
            ui.heading("Timeline");
            ui.separator();
            ui.horizontal(|ui| {
                if ui.button("▶").clicked() {
                    // Play
                }
                if ui.button("⏸").clicked() {
                    // Pause
                }
                if ui.button("⏹").clicked() {
                    // Stop
                }
            });
        });

    egui::CentralPanel::default().show(contexts.ctx_mut(), |ui| {
        ui.centered_and_justified(|ui| {
            ui.label("Canvas - Create or open a project to begin");
        });
    });
}
