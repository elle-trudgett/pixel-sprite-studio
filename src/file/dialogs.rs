use std::path::PathBuf;

#[cfg(target_os = "windows")]
use rfd::FileDialog;

// Native file dialog functions (Windows only)
#[cfg(target_os = "windows")]
pub fn pick_save_file() -> Option<PathBuf> {
    FileDialog::new()
        .add_filter("Pixel Sprite Studio Project", &["pss"])
        .set_file_name("project.pss")
        .save_file()
}

#[cfg(target_os = "windows")]
pub fn pick_file() -> Option<PathBuf> {
    FileDialog::new()
        .add_filter("Pixel Sprite Studio Project", &["pss"])
        .pick_file()
}

#[cfg(target_os = "windows")]
pub fn pick_image_file() -> Option<PathBuf> {
    FileDialog::new()
        .add_filter("PNG Images", &["png"])
        .add_filter("All Images", &["png", "jpg", "jpeg", "gif", "bmp"])
        .pick_file()
}

#[cfg(target_os = "windows")]
pub fn pick_export_file() -> Option<PathBuf> {
    FileDialog::new()
        .add_filter("PNG Image", &["png"])
        .set_file_name("spritesheet.png")
        .save_file()
}

#[cfg(target_os = "windows")]
pub fn pick_export_folder() -> Option<PathBuf> {
    FileDialog::new().pick_folder()
}

// Fallback for non-Windows (returns None, uses text input instead)
#[cfg(not(target_os = "windows"))]
pub fn pick_save_file() -> Option<PathBuf> {
    None
}
#[cfg(not(target_os = "windows"))]
pub fn pick_file() -> Option<PathBuf> {
    None
}
#[cfg(not(target_os = "windows"))]
pub fn pick_image_file() -> Option<PathBuf> {
    None
}
#[cfg(not(target_os = "windows"))]
pub fn pick_export_file() -> Option<PathBuf> {
    None
}
#[cfg(not(target_os = "windows"))]
pub fn pick_export_folder() -> Option<PathBuf> {
    None
}
