// file_handler.rs
use std::path::PathBuf;
use crate::models::AppError;

pub struct FileHandler;

impl FileHandler {
    pub fn load_text_file() -> Option<String> {
        let path = rfd::FileDialog::new()
            .add_filter("Text files", &["txt", "md", "rs", "py", "js", "json"])
            .pick_file()?;
        
        std::fs::read_to_string(&path).ok()
    }

    pub fn save_text_file(content: &str, default_name: &str) -> Result<(), AppError> {
        let path = rfd::FileDialog::new()
            .set_file_name(default_name)
            .save_file()
            .ok_or_else(|| AppError("No file selected".to_string()))?;
        
        std::fs::write(&path, content)?;
        Ok(())
    }

    pub fn open_directory(path: &PathBuf) {
        #[cfg(target_os = "windows")]
        std::process::Command::new("explorer")
            .arg(path)
            .spawn()
            .ok();
        
        #[cfg(target_os = "macos")]
        std::process::Command::new("open")
            .arg(path)
            .spawn()
            .ok();
        
        #[cfg(target_os = "linux")]
        std::process::Command::new("xdg-open")
            .arg(path)
            .spawn()
            .ok();
    }

    pub fn create_prompt_with_file_context(file_content: &str, input_text: &str) -> String {
        if file_content.is_empty() {
            input_text.to_string()
        } else {
            format!("{}\n\n{}", file_content, input_text)
        }
    }
}