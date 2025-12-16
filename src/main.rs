use std::fs;
use std::path::PathBuf;
use std::rc::Rc;
use std::cell::RefCell;

use arboard::Clipboard;
use slint::{Image, Rgba8Pixel, SharedPixelBuffer, ModelRc, VecModel, SharedString};

slint::include_modules!();

const NUM_SLOTS: usize = 5;

/// Find the data directory containing emoji files.
/// Searches in order:
/// 1. ./data (next to executable)
/// 2. ../data (for development)
/// 3. ~/.emoru/data
/// 4. ~/emoji_picker_images (legacy)
fn find_data_dir() -> Option<PathBuf> {
    // Next to executable
    if let Ok(exe) = std::env::current_exe() {
        let exe_dir = exe.parent().unwrap_or(&exe);
        let data_dir = exe_dir.join("data");
        if data_dir.exists() {
            return Some(data_dir);
        }
    }

    // Development: ./data
    let local_data = PathBuf::from("data");
    if local_data.exists() {
        return Some(local_data);
    }

    // User config: ~/.emoru/data
    if let Some(home) = dirs::home_dir() {
        let user_data = home.join(".emoru").join("data");
        if user_data.exists() {
            return Some(user_data);
        }

        // Legacy location
        let legacy = home.join("emoji_picker_images");
        if legacy.exists() {
            // For legacy, return parent since we expect data/emoji_picker_images structure
            return Some(home.clone());
        }
    }

    None
}

struct AppState {
    emojis: Vec<String>,
    letters: Vec<char>,
    matches: Vec<String>,
    selected_index: i32,
    selected_emoji: Option<String>,
    image_cache: std::collections::HashMap<String, Image>,
    data_dir: Option<PathBuf>,
}

impl AppState {
    fn new() -> Self {
        Self {
            emojis: Vec::new(),
            letters: Vec::new(),
            matches: Vec::new(),
            selected_index: 0,
            selected_emoji: None,
            image_cache: std::collections::HashMap::new(),
            data_dir: find_data_dir(),
        }
    }

    fn load_emojis(&mut self) {
        if self.emojis.is_empty() {
            // Try bundled data first
            if let Some(ref data_dir) = self.data_dir {
                let path = data_dir.join("emojis9.txt");
                if let Ok(content) = fs::read_to_string(&path) {
                    self.emojis = content.lines().map(String::from).collect();
                    return;
                }
            }

            // Fallback to home directory (legacy)
            if let Some(home) = dirs::home_dir() {
                let path = home.join("emojis9.txt");
                if let Ok(content) = fs::read_to_string(&path) {
                    self.emojis = content.lines().map(String::from).collect();
                }
            }
        }
    }

    fn search_text(&self) -> String {
        self.letters.iter().collect()
    }

    fn search(&mut self) {
        let query: String = self.letters.iter().collect::<String>().to_lowercase();

        self.matches = self.emojis
            .iter()
            .filter(|e| e.to_lowercase().contains(&query))
            .take(NUM_SLOTS)
            .cloned()
            .collect();

        if self.matches.is_empty() && !self.emojis.is_empty() {
            self.matches = self.emojis.iter().take(NUM_SLOTS).cloned().collect();
        }

        // Ensure selected_index is within bounds
        let max_idx = (self.matches.len() as i32 - 1).max(0);
        self.selected_index = self.selected_index.min(max_idx);
    }

    fn load_image(&mut self, code: &str) -> Option<Image> {
        if let Some(img) = self.image_cache.get(code) {
            return Some(img.clone());
        }

        // Try bundled data first
        let path = if let Some(ref data_dir) = self.data_dir {
            data_dir.join("emoji_picker_images").join(format!("{}.base64", code))
        } else {
            // Fallback to legacy location
            dirs::home_dir()
                .unwrap_or_default()
                .join("emoji_picker_images")
                .join(format!("{}.base64", code))
        };

        if let Ok(b64_data) = fs::read_to_string(&path) {
            if let Ok(img_data) = base64::Engine::decode(
                &base64::engine::general_purpose::STANDARD,
                b64_data.trim()
            ) {
                if let Ok(img) = image::load_from_memory(&img_data) {
                    let rgba = img.to_rgba8();
                    let (width, height) = rgba.dimensions();
                    let buffer = SharedPixelBuffer::<Rgba8Pixel>::clone_from_slice(
                        rgba.as_raw(),
                        width,
                        height,
                    );
                    let slint_image = Image::from_rgba8(buffer);
                    self.image_cache.insert(code.to_string(), slint_image.clone());
                    return Some(slint_image);
                }
            }
        }
        None
    }

    fn get_emoji_entries(&mut self) -> Vec<EmojiEntry> {
        let mut entries = Vec::new();
        let query: String = self.letters.iter().collect::<String>().to_lowercase();

        for entry in &self.matches.clone() {
            let parts: Vec<&str> = entry.split("| ").collect();
            if parts.len() >= 3 {
                let emoji = parts[0].to_string();
                let description = parts[1].to_string();
                let code = parts[2].to_string();
                let image_data = self.load_image(&code).unwrap_or_default();

                // Find match position for highlighting
                let (prefix, match_text, suffix) = if !query.is_empty() {
                    if let Some(pos) = description.to_lowercase().find(&query) {
                        let p = description[..pos].to_string();
                        let m = description[pos..pos + query.len()].to_string();
                        let s = description[pos + query.len()..].to_string();
                        (p, m, s)
                    } else {
                        (description.clone(), String::new(), String::new())
                    }
                } else {
                    (description.clone(), String::new(), String::new())
                };

                entries.push(EmojiEntry {
                    emoji: SharedString::from(emoji),
                    description: SharedString::from(description),
                    prefix: SharedString::from(prefix),
                    match_text: SharedString::from(match_text),
                    suffix: SharedString::from(suffix),
                    image_data,
                });
            } else {
                entries.push(EmojiEntry {
                    emoji: SharedString::default(),
                    description: SharedString::default(),
                    prefix: SharedString::default(),
                    match_text: SharedString::default(),
                    suffix: SharedString::default(),
                    image_data: Image::default(),
                });
            }
        }

        entries
    }
}

fn main() -> Result<(), slint::PlatformError> {
    // Suppress Qt warnings (including thread cleanup warnings)
    std::env::set_var("QT_LOGGING_RULES", "*=false");
    std::env::set_var("QT_MESSAGE_PATTERN", "");

    let app = EmojiPicker::new()?;
    let state = Rc::new(RefCell::new(AppState::new()));

    // Load emojis on startup
    state.borrow_mut().load_emojis();
    state.borrow_mut().search();

    // Initial update
    update_ui(&app, &state);

    // Handle key presses
    let app_weak = app.as_weak();
    let state_clone = state.clone();
    app.on_key_pressed(move |key| {
        let mut state = state_clone.borrow_mut();
        let key_str = key.as_str();

        match key_str {
            "up" => {
                state.selected_index = (state.selected_index - 1).max(0);
            }
            "down" => {
                let max_idx = (state.matches.len() as i32 - 1).max(0);
                state.selected_index = (state.selected_index + 1).min(max_idx);
            }
            "backspace" => {
                state.letters.pop();
                state.selected_index = 0;
                state.search();
            }
            "ctrl-backspace" => {
                state.letters.clear();
                state.selected_index = 0;
                state.search();
            }
            _ => {
                // Regular character
                for c in key_str.chars() {
                    state.letters.push(c);
                }
                state.selected_index = 0;
                state.search();
            }
        }

        drop(state);
        if let Some(app) = app_weak.upgrade() {
            update_ui(&app, &state_clone);
        }
    });

    // Handle emoji selection
    let state_clone = state.clone();
    let app_weak = app.as_weak();
    app.on_emoji_selected(move |emoji| {
        state_clone.borrow_mut().selected_emoji = Some(emoji.to_string());
        if let Some(app) = app_weak.upgrade() {
            app.hide().ok();
        }
    });

    // Handle close
    let app_weak = app.as_weak();
    app.on_close_requested(move || {
        if let Some(app) = app_weak.upgrade() {
            app.hide().ok();
        }
    });

    app.run()?;

    // After window closes, paste the emoji if one was selected
    if let Some(emoji) = state.borrow().selected_emoji.clone() {
        paste_emoji(&emoji);
    }

    Ok(())
}

fn update_ui(app: &EmojiPicker, state: &Rc<RefCell<AppState>>) {
    let mut state = state.borrow_mut();

    app.set_search_text(SharedString::from(state.search_text()));
    app.set_selected_index(state.selected_index);

    let entries = state.get_emoji_entries();
    let model = Rc::new(VecModel::from(entries));
    app.set_emoji_entries(ModelRc::from(model));
}

fn paste_emoji(emoji: &str) {
    let mut clipboard = match Clipboard::new() {
        Ok(c) => c,
        Err(_) => return,
    };

    // Save current clipboard
    let previous = clipboard.get_text().ok();

    // Set emoji
    if clipboard.set_text(emoji).is_ok() {
        // Simulate Ctrl+V (platform-specific)
        #[cfg(target_os = "linux")]
        {
            use std::process::Command;
            // Use xdotool for X11 or wtype for Wayland
            let _ = Command::new("xdotool")
                .args(["key", "ctrl+v"])
                .status();
        }

        #[cfg(target_os = "macos")]
        {
            use std::process::Command;
            let _ = Command::new("osascript")
                .args(["-e", "tell application \"System Events\" to keystroke \"v\" using command down"])
                .status();
        }

        // Restore clipboard after a short delay
        std::thread::sleep(std::time::Duration::from_millis(100));
        if let Some(prev) = previous {
            let _ = clipboard.set_text(prev);
        }
    }
}
