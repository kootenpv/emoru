use std::fs::{self, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use std::rc::Rc;
use std::cell::RefCell;
use std::time::{SystemTime, UNIX_EPOCH};
use std::collections::HashMap;

use arboard::Clipboard;
use serde::{Deserialize, Serialize};
use slint::{Image, Rgba8Pixel, SharedPixelBuffer, ModelRc, VecModel, SharedString};

slint::include_modules!();

#[derive(Serialize, Deserialize)]
#[serde(tag = "type")]
enum LogEvent {
    #[serde(rename = "keystroke")]
    Keystroke { ts: u64, key: String },
    #[serde(rename = "select")]
    Select { ts: u64, code: String, query: String },
}

fn get_log_path() -> Option<PathBuf> {
    dirs::home_dir().map(|h| h.join("emoru_strokes.jsonl"))
}

fn log_event(event: &LogEvent) {
    if let Some(path) = get_log_path() {
        if let Ok(mut file) = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
        {
            if let Ok(json) = serde_json::to_string(event) {
                let _ = writeln!(file, "{}", json);
            }
        }
    }
}

fn current_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// A recorded emoji selection with query context
#[derive(Clone)]
struct Selection {
    code: String,
    query: String,
    ts: u64,
}

/// Load all selections from the log file
fn load_selections() -> Vec<Selection> {
    let mut selections = Vec::new();

    if let Some(path) = get_log_path() {
        if let Ok(file) = fs::File::open(&path) {
            let reader = BufReader::new(file);
            for line in reader.lines().map_while(Result::ok) {
                if let Ok(event) = serde_json::from_str::<LogEvent>(&line) {
                    if let LogEvent::Select { ts, code, query } = event {
                        selections.push(Selection { code, query: query.to_lowercase(), ts });
                    }
                }
            }
        }
    }

    selections
}

/// Check if two queries are prefix-related (one is prefix of the other)
fn queries_match(current: &str, stored: &str) -> bool {
    current.starts_with(stored) || stored.starts_with(current)
}

/// Calculate frecency scores for a given query, considering query-prefix matching
fn compute_frecency_for_query(selections: &[Selection], current_query: &str) -> HashMap<String, f64> {
    let mut scores: HashMap<String, f64> = HashMap::new();
    let now = current_timestamp();
    let half_life_secs: f64 = 7.0 * 24.0 * 60.0 * 60.0; // 7 days

    let current_lower = current_query.to_lowercase();

    for sel in selections {
        // Only count selections where the stored query is prefix-related to current query
        if current_lower.is_empty() || queries_match(&current_lower, &sel.query) {
            let age_secs = (now.saturating_sub(sel.ts)) as f64;
            let decay = 0.5_f64.powf(age_secs / half_life_secs);
            *scores.entry(sel.code.clone()).or_insert(0.0) += decay;
        }
    }

    scores
}

const NUM_SLOTS: usize = 5;

/// Check if a search term matches a word using fuzzy prefix matching:
/// - First char of term must match first char of word
/// - Remaining chars must appear in order (subsequence) in the word
fn term_matches_word(term: &str, word: &str) -> bool {
    let mut term_chars = term.chars();
    let mut word_chars = word.chars();

    // First char must match word start
    match (term_chars.next(), word_chars.next()) {
        (Some(tc), Some(wc)) if tc == wc => {}
        (None, _) => return true, // empty term matches everything
        _ => return false,
    }

    // Remaining chars must appear in order (subsequence)
    for tc in term_chars {
        loop {
            match word_chars.next() {
                Some(wc) if wc == tc => break,
                Some(_) => continue,
                None => return false,
            }
        }
    }

    true
}

/// Check if an emoji entry matches all search terms
/// Each term must match at least one word in the description
fn entry_matches_terms(entry: &str, terms: &[&str]) -> bool {
    let parts: Vec<&str> = entry.split("| ").collect();
    if parts.len() < 2 {
        return false;
    }
    let description = parts[1].to_lowercase();
    let words: Vec<&str> = description.split_whitespace().collect();

    // Each term must match at least one word
    terms.iter().all(|term| {
        if term.is_empty() {
            return true;
        }
        words.iter().any(|word| term_matches_word(term, word))
    })
}

/// Find character indices that match a term using fuzzy subsequence matching
/// Returns None if no match, or Some(indices) of matched characters in word
fn find_fuzzy_match_indices(term: &str, word: &str) -> Option<Vec<usize>> {
    let mut term_chars = term.chars().peekable();
    let mut indices = Vec::new();

    // First char must match word start
    let first_term = term_chars.next()?;
    let mut word_iter = word.char_indices();
    let (first_idx, first_word) = word_iter.next()?;

    if first_term.to_lowercase().next()? != first_word.to_lowercase().next()? {
        return None;
    }
    indices.push(first_idx);

    // Remaining chars must appear in order
    for tc in term_chars {
        let tc_lower = tc.to_lowercase().next()?;
        loop {
            match word_iter.next() {
                Some((idx, wc)) => {
                    if wc.to_lowercase().next()? == tc_lower {
                        indices.push(idx);
                        break;
                    }
                }
                None => return None,
            }
        }
    }

    Some(indices)
}

/// Build text segments with highlighted (bold) matches for fuzzy prefix matching
fn build_highlight_segments(text: &str, terms: &[&str]) -> Vec<TextSegment> {
    if terms.is_empty() || terms.iter().all(|t| t.is_empty()) {
        return vec![TextSegment {
            text: SharedString::from(text),
            bold: false,
        }];
    }

    // Find all character positions to highlight
    let mut highlight_positions: Vec<bool> = vec![false; text.len()];

    // Split into words with their positions
    let mut word_start = 0;
    for word in text.split_whitespace() {
        // Find actual position of word in text
        if let Some(pos) = text[word_start..].find(word) {
            let abs_start = word_start + pos;
            let word_lower = word.to_lowercase();

            // Check each term against this word
            for term in terms {
                if term.is_empty() {
                    continue;
                }
                if let Some(indices) = find_fuzzy_match_indices(term, &word_lower) {
                    // Mark these character positions as highlighted
                    for idx in indices {
                        let abs_idx = abs_start + idx;
                        if abs_idx < highlight_positions.len() {
                            highlight_positions[abs_idx] = true;
                        }
                    }
                }
            }
            word_start = abs_start + word.len();
        }
    }

    // Build segments from highlight positions
    let mut segments = Vec::new();
    let mut current_text = String::new();
    let mut current_bold = false;
    let mut first = true;

    for (i, ch) in text.char_indices() {
        let should_bold = highlight_positions.get(i).copied().unwrap_or(false);

        if first {
            current_bold = should_bold;
            first = false;
        }

        if should_bold != current_bold {
            if !current_text.is_empty() {
                segments.push(TextSegment {
                    text: SharedString::from(&current_text),
                    bold: current_bold,
                });
                current_text.clear();
            }
            current_bold = should_bold;
        }
        current_text.push(ch);
    }

    if !current_text.is_empty() {
        segments.push(TextSegment {
            text: SharedString::from(&current_text),
            bold: current_bold,
        });
    }

    if segments.is_empty() {
        segments.push(TextSegment {
            text: SharedString::from(text),
            bold: false,
        });
    }

    segments
}

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
    image_cache: HashMap<String, Image>,
    data_dir: Option<PathBuf>,
    selections: Vec<Selection>,
}

impl AppState {
    fn new() -> Self {
        Self {
            emojis: Vec::new(),
            letters: Vec::new(),
            matches: Vec::new(),
            selected_index: 0,
            selected_emoji: None,
            image_cache: HashMap::new(),
            data_dir: find_data_dir(),
            selections: load_selections(),
        }
    }

    /// Get the emoji code from an entry string
    fn get_code(entry: &str) -> Option<String> {
        let parts: Vec<&str> = entry.split("| ").collect();
        if parts.len() >= 3 {
            Some(parts[2].to_string())
        } else {
            None
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
        let terms: Vec<&str> = query.split_whitespace().collect();

        // Compute frecency scores based on current query prefix
        let frecency = compute_frecency_for_query(&self.selections, &query);

        // Helper to get frecency for an entry
        let get_frecency = |entry: &str| -> f64 {
            if let Some(code) = Self::get_code(entry) {
                *frecency.get(&code).unwrap_or(&0.0)
            } else {
                0.0
            }
        };

        // Filter matching emojis using fuzzy prefix matching
        let mut filtered: Vec<String> = self.emojis
            .iter()
            .filter(|e| entry_matches_terms(e, &terms))
            .cloned()
            .collect();

        // Sort by frecency (highest first)
        filtered.sort_by(|a, b| {
            let fa = get_frecency(a);
            let fb = get_frecency(b);
            fb.partial_cmp(&fa).unwrap_or(std::cmp::Ordering::Equal)
        });

        self.matches = filtered.into_iter().take(NUM_SLOTS).collect();

        if self.matches.is_empty() && !self.emojis.is_empty() {
            // Show top frecency emojis when no query (all selections count)
            let mut top: Vec<String> = self.emojis.clone();
            top.sort_by(|a, b| {
                let fa = get_frecency(a);
                let fb = get_frecency(b);
                fb.partial_cmp(&fa).unwrap_or(std::cmp::Ordering::Equal)
            });
            self.matches = top.into_iter().take(NUM_SLOTS).collect();
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
        let terms: Vec<&str> = query.split_whitespace().collect();

        for entry in &self.matches.clone() {
            let parts: Vec<&str> = entry.split("| ").collect();
            if parts.len() >= 3 {
                let emoji = parts[0].to_string();
                let description = parts[1].to_string();
                let code = parts[2].to_string();
                let image_data = self.load_image(&code).unwrap_or_default();

                // Build segments with multi-term highlighting
                let segments = build_highlight_segments(&description, &terms);

                entries.push(EmojiEntry {
                    emoji: SharedString::from(emoji),
                    description: SharedString::from(description),
                    segments: ModelRc::from(Rc::new(VecModel::from(segments))),
                    image_data,
                });
            } else {
                entries.push(EmojiEntry {
                    emoji: SharedString::default(),
                    description: SharedString::default(),
                    segments: ModelRc::default(),
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

        // Log keystroke
        log_event(&LogEvent::Keystroke {
            ts: current_timestamp(),
            key: key_str.to_string(),
        });

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
            "shift" => {
                // Swallow shift key, don't output anything
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
        let mut state = state_clone.borrow_mut();

        // Log selection with code and query
        let query: String = state.letters.iter().collect();
        let idx = state.selected_index as usize;
        if let Some(entry) = state.matches.get(idx) {
            let parts: Vec<&str> = entry.split("| ").collect();
            if parts.len() >= 3 {
                let code = parts[2].to_string();
                log_event(&LogEvent::Select {
                    ts: current_timestamp(),
                    code,
                    query,
                });
            }
        }

        state.selected_emoji = Some(emoji.to_string());
        drop(state);
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
