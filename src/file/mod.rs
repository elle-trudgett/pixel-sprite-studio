mod dialogs;

pub use dialogs::{
    pick_export_file, pick_export_folder, pick_file, pick_image_file, pick_save_file,
};

/// Alias for pick_file for semantic clarity when opening
pub fn pick_open_file() -> Option<std::path::PathBuf> {
    pick_file()
}
