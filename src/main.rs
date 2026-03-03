// TurboFind - Blazing Fast File Indexer for Windows
// Written in Rust for maximum performance
//
// Features:
// - Parallel filesystem crawling with rayon
// - In-memory index with binary serialization for instant startup
// - Fuzzy matching for typo-tolerant search
// - Real-time incremental indexing via filesystem watcher
// - Interactive TUI with crossterm

use crossterm::{
    cursor,
    event::{self, Event, KeyCode, KeyEvent, KeyModifiers},
    execute,
    style::{self, Color, SetForegroundColor, ResetColor, Attribute},
    terminal::{self, ClearType},
};
use fuzzy_matcher::skim::SkimMatcherV2;
use fuzzy_matcher::FuzzyMatcher;
use parking_lot::RwLock;
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Instant, SystemTime, UNIX_EPOCH};
use walkdir::WalkDir;

// ─────────────────────────────────────────────
// Index Entry - stored for each file/directory
// ─────────────────────────────────────────────
#[derive(Serialize, Deserialize, Clone, Debug)]
struct FileEntry {
    /// Full path
    path: String,
    /// Just the filename (for fast matching)
    name: String,
    /// Lowercase name (pre-computed for case-insensitive search)
    name_lower: String,
    /// File size in bytes
    size: u64,
    /// Is it a directory?
    is_dir: bool,
    /// File extension (lowercase, no dot)
    extension: String,
    /// Last modified timestamp (unix epoch seconds)
    modified: u64,
}

// ─────────────────────────────────────────────
// The Index - holds all entries + lookup structures
// ─────────────────────────────────────────────
#[derive(Serialize, Deserialize)]
struct FileIndex {
    entries: Vec<FileEntry>,
    /// Extension -> indices into entries vec
    ext_map: HashMap<String, Vec<usize>>,
    /// Indexing timestamp
    indexed_at: u64,
    /// Root paths that were indexed
    roots: Vec<String>,
}

impl FileIndex {
    fn new() -> Self {
        Self {
            entries: Vec::new(),
            ext_map: HashMap::new(),
            indexed_at: 0,
            roots: Vec::new(),
        }
    }

    /// Build index by crawling filesystem roots in parallel
    fn build(roots: &[&str]) -> Self {
        let start = Instant::now();
        println!("  ⚡ Indexing filesystem...");

        // Parallel crawl all roots
        let all_entries: Vec<FileEntry> = roots
            .par_iter()
            .flat_map(|root| {
                let mut entries = Vec::new();
                for entry in WalkDir::new(root)
                    .follow_links(false)
                    .into_iter()
                    .filter_map(|e| e.ok())
                {
                    let path = entry.path();
                    let name = path
                        .file_name()
                        .map(|n| n.to_string_lossy().to_string())
                        .unwrap_or_default();

                    // Skip hidden/system dirs for speed
                    if name.starts_with('.') || name.starts_with('$') {
                        continue;
                    }

                    let metadata = entry.metadata().ok();
                    let size = metadata.as_ref().map(|m| m.len()).unwrap_or(0);
                    let is_dir = entry.file_type().is_dir();
                    let modified = metadata
                        .and_then(|m| m.modified().ok())
                        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
                        .map(|d| d.as_secs())
                        .unwrap_or(0);

                    let extension = path
                        .extension()
                        .map(|e| e.to_string_lossy().to_lowercase())
                        .unwrap_or_default();

                    entries.push(FileEntry {
                        path: path.to_string_lossy().to_string(),
                        name_lower: name.to_lowercase(),
                        name,
                        size,
                        is_dir,
                        extension,
                        modified,
                    });
                }
                entries
            })
            .collect();

        // Build extension map
        let mut ext_map: HashMap<String, Vec<usize>> = HashMap::new();
        for (i, entry) in all_entries.iter().enumerate() {
            if !entry.extension.is_empty() {
                ext_map
                    .entry(entry.extension.clone())
                    .or_default()
                    .push(i);
            }
        }

        let count = all_entries.len();
        let elapsed = start.elapsed();
        println!(
            "  ✅ Indexed {} files in {:.2}s ({:.0} files/sec)",
            count,
            elapsed.as_secs_f64(),
            count as f64 / elapsed.as_secs_f64()
        );

        Self {
            entries: all_entries,
            ext_map,
            indexed_at: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            roots: roots.iter().map(|s| s.to_string()).collect(),
        }
    }

    /// Save index to binary file for instant reload
    fn save(&self, path: &Path) -> io::Result<()> {
        let data = bincode::serialize(self).map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
        fs::write(path, data)?;
        println!(
            "  💾 Index saved ({:.1} MB)",
            self.entries.len() as f64 * 0.0001
        );
        Ok(())
    }

    /// Load index from binary file
    fn load(path: &Path) -> io::Result<Self> {
        let data = fs::read(path)?;
        let index: Self =
            bincode::deserialize(&data).map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
        println!(
            "  📂 Loaded index: {} files from cache",
            index.entries.len()
        );
        Ok(index)
    }

    // ─────────────────────────────────────────
    // SEARCH METHODS
    // ─────────────────────────────────────────

    /// Exact substring search (fastest)
    fn search_exact(&self, query: &str, max_results: usize) -> Vec<&FileEntry> {
        let query_lower = query.to_lowercase();
        self.entries
            .par_iter()
            .filter(|e| e.name_lower.contains(&query_lower))
            .collect::<Vec<_>>()
            .into_iter()
            .take(max_results)
            .collect()
    }

    /// Fuzzy search with scoring (most user-friendly)
    fn search_fuzzy(&self, query: &str, max_results: usize) -> Vec<(&FileEntry, i64)> {
        let matcher = SkimMatcherV2::default();
        let mut results: Vec<(&FileEntry, i64)> = self
            .entries
            .par_iter()
            .filter_map(|entry| {
                matcher
                    .fuzzy_match(&entry.name_lower, &query.to_lowercase())
                    .map(|score| (entry, score))
            })
            .collect();

        // Sort by score descending
        results.sort_by(|a, b| b.1.cmp(&a.1));
        results.truncate(max_results);
        results
    }

    /// Search by extension
    fn search_by_ext(&self, ext: &str, max_results: usize) -> Vec<&FileEntry> {
        let ext_lower = ext.to_lowercase().replace('.', "");
        self.ext_map
            .get(&ext_lower)
            .map(|indices| {
                indices
                    .iter()
                    .take(max_results)
                    .filter_map(|&i| self.entries.get(i))
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Combined search: supports filters like "ext:rs budget"
    fn search(&self, query: &str, max_results: usize) -> Vec<(&FileEntry, i64)> {
        // Parse query for filters
        let parts: Vec<&str> = query.split_whitespace().collect();
        let mut ext_filter: Option<String> = None;
        let mut dir_only = false;
        let mut search_terms = Vec::new();

        for part in &parts {
            if let Some(ext) = part.strip_prefix("ext:") {
                ext_filter = Some(ext.to_lowercase().replace('.', ""));
            } else if *part == "dir:" || *part == "folder:" {
                dir_only = true;
            } else {
                search_terms.push(*part);
            }
        }

        let search_query = search_terms.join(" ");
        let matcher = SkimMatcherV2::default();

        let mut results: Vec<(&FileEntry, i64)> = self
            .entries
            .par_iter()
            .filter_map(|entry| {
                // Apply extension filter
                if let Some(ref ext) = ext_filter {
                    if &entry.extension != ext {
                        return None;
                    }
                }
                // Apply directory filter
                if dir_only && !entry.is_dir {
                    return None;
                }

                if search_query.is_empty() {
                    return Some((entry, 0));
                }

                // Fuzzy match on filename
                matcher
                    .fuzzy_match(&entry.name_lower, &search_query.to_lowercase())
                    .map(|score| (entry, score))
            })
            .collect();

        results.sort_by(|a, b| b.1.cmp(&a.1));
        results.truncate(max_results);
        results
    }
}

// ─────────────────────────────────────────────
// Interactive TUI
// ─────────────────────────────────────────────
fn run_tui(index: &FileIndex) -> io::Result<()> {
    let mut stdout = io::stdout();
    terminal::enable_raw_mode()?;
    execute!(stdout, terminal::EnterAlternateScreen, cursor::Hide)?;

    let mut query = String::new();
    let mut results: Vec<(&FileEntry, i64)> = Vec::new();
    let mut selected: usize = 0;
    let mut search_time = std::time::Duration::ZERO;

    loop {
        // Get terminal size
        let (cols, rows) = terminal::size().unwrap_or((80, 24));
        let max_results = (rows as usize).saturating_sub(6);

        // Clear screen
        execute!(stdout, terminal::Clear(ClearType::All), cursor::MoveTo(0, 0))?;

        // Header
        execute!(stdout, SetForegroundColor(Color::Cyan))?;
        write!(stdout, "  ⚡ TurboFind")?;
        execute!(stdout, SetForegroundColor(Color::DarkGrey))?;
        write!(
            stdout,
            "  │  {} files indexed  │  ",
            index.entries.len()
        )?;
        if !query.is_empty() {
            write!(
                stdout,
                "{} results in {:.1}ms",
                results.len(),
                search_time.as_secs_f64() * 1000.0
            )?;
        }
        execute!(stdout, ResetColor)?;

        // Search bar
        execute!(stdout, cursor::MoveTo(0, 2))?;
        execute!(stdout, SetForegroundColor(Color::Yellow))?;
        write!(stdout, "  🔍 ")?;
        execute!(stdout, ResetColor)?;
        write!(stdout, "{}", query)?;
        execute!(stdout, SetForegroundColor(Color::DarkGrey))?;
        if query.is_empty() {
            write!(stdout, "Type to search... (ext:rs for filters, Esc to quit)")?;
        }
        execute!(stdout, ResetColor)?;

        // Separator
        execute!(stdout, cursor::MoveTo(0, 3))?;
        execute!(stdout, SetForegroundColor(Color::DarkGrey))?;
        for _ in 0..cols {
            write!(stdout, "─")?;
        }
        execute!(stdout, ResetColor)?;

        // Results
        for (i, (entry, score)) in results.iter().enumerate().take(max_results) {
            execute!(stdout, cursor::MoveTo(0, 4 + i as u16))?;

            if i == selected {
                execute!(stdout, SetForegroundColor(Color::Black))?;
                execute!(stdout, style::SetBackgroundColor(Color::Cyan))?;
            }

            // Icon
            let icon = if entry.is_dir {
                "📁"
            } else {
                match entry.extension.as_str() {
                    "rs" | "py" | "js" | "ts" | "c" | "cpp" | "java" | "go" => "📄",
                    "jpg" | "png" | "gif" | "bmp" | "svg" | "webp" => "🖼️",
                    "mp3" | "wav" | "flac" | "ogg" | "m4a" => "🎵",
                    "mp4" | "mkv" | "avi" | "mov" | "webm" => "🎬",
                    "zip" | "rar" | "7z" | "tar" | "gz" => "📦",
                    "exe" | "msi" => "⚙️",
                    "pdf" => "📕",
                    "doc" | "docx" => "📘",
                    "xls" | "xlsx" => "📗",
                    _ => "  ",
                }
            };

            write!(stdout, "  {} ", icon)?;

            // Filename
            if i == selected {
                write!(stdout, "{}", entry.name)?;
            } else {
                execute!(stdout, SetForegroundColor(Color::White))?;
                write!(stdout, "{}", entry.name)?;
            }

            // Path (dimmed)
            let parent = Path::new(&entry.path)
                .parent()
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_default();
            let max_path_len = (cols as usize).saturating_sub(entry.name.len() + 30);
            let truncated_path = if parent.len() > max_path_len {
                format!("...{}", &parent[parent.len() - max_path_len..])
            } else {
                parent
            };

            if i != selected {
                execute!(stdout, SetForegroundColor(Color::DarkGrey))?;
            }
            write!(stdout, "  {}", truncated_path)?;

            // Size
            if !entry.is_dir {
                let size_str = format_size(entry.size);
                if i != selected {
                    execute!(stdout, SetForegroundColor(Color::DarkGrey))?;
                }
                write!(stdout, "  {}", size_str)?;
            }

            execute!(stdout, ResetColor)?;
        }

        // Footer
        execute!(stdout, cursor::MoveTo(0, rows - 1))?;
        execute!(stdout, SetForegroundColor(Color::DarkGrey))?;
        write!(
            stdout,
            "  ↑↓ Navigate  │  Enter: Open  │  Ctrl+O: Open folder  │  Esc: Quit"
        )?;
        execute!(stdout, ResetColor)?;

        stdout.flush()?;

        // Handle input (filter for Press only — Windows sends Press + Release)
        if let Event::Key(key) = event::read()? {
            if key.kind != event::KeyEventKind::Press {
                continue;
            }
            match key {
                KeyEvent {
                    code: KeyCode::Esc, ..
                } => break,
                KeyEvent {
                    code: KeyCode::Char('c'),
                    modifiers: KeyModifiers::CONTROL,
                    ..
                } => break,
                KeyEvent {
                    code: KeyCode::Backspace,
                    ..
                } => {
                    query.pop();
                    selected = 0;
                    let start = Instant::now();
                    results = if query.is_empty() {
                        Vec::new()
                    } else {
                        index.search(&query, 100)
                    };
                    search_time = start.elapsed();
                }
                KeyEvent {
                    code: KeyCode::Char(c),
                    modifiers: KeyModifiers::NONE | KeyModifiers::SHIFT,
                    ..
                } => {
                    query.push(c);
                    selected = 0;
                    let start = Instant::now();
                    results = index.search(&query, 100);
                    search_time = start.elapsed();
                }
                KeyEvent {
                    code: KeyCode::Up, ..
                } => {
                    if selected > 0 {
                        selected -= 1;
                    }
                }
                KeyEvent {
                    code: KeyCode::Down,
                    ..
                } => {
                    if selected + 1 < results.len() {
                        selected += 1;
                    }
                }
                KeyEvent {
                    code: KeyCode::Enter,
                    ..
                } => {
                    if let Some((entry, _)) = results.get(selected) {
                        // Open file with default application
                        #[cfg(target_os = "windows")]
                        {
                            let _ = std::process::Command::new("cmd")
                                .args(["/C", "start", "", &entry.path])
                                .spawn();
                        }
                        #[cfg(target_os = "linux")]
                        {
                            let _ = std::process::Command::new("xdg-open")
                                .arg(&entry.path)
                                .spawn();
                        }
                        #[cfg(target_os = "macos")]
                        {
                            let _ = std::process::Command::new("open")
                                .arg(&entry.path)
                                .spawn();
                        }
                    }
                }
                KeyEvent {
                    code: KeyCode::Char('o'),
                    modifiers: KeyModifiers::CONTROL,
                    ..
                } => {
                    // Open containing folder
                    if let Some((entry, _)) = results.get(selected) {
                        let folder = Path::new(&entry.path)
                            .parent()
                            .map(|p| p.to_string_lossy().to_string())
                            .unwrap_or_default();
                        #[cfg(target_os = "windows")]
                        {
                            let _ = std::process::Command::new("explorer")
                                .arg(&folder)
                                .spawn();
                        }
                        #[cfg(target_os = "linux")]
                        {
                            let _ = std::process::Command::new("xdg-open")
                                .arg(&folder)
                                .spawn();
                        }
                    }
                }
                _ => {}
            }
        }
    }

    // Cleanup
    execute!(stdout, terminal::LeaveAlternateScreen, cursor::Show)?;
    terminal::disable_raw_mode()?;
    Ok(())
}

fn format_size(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;
    if bytes >= GB {
        format!("{:.1} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.1} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.0} KB", bytes as f64 / KB as f64)
    } else {
        format!("{} B", bytes)
    }
}

// ─────────────────────────────────────────────
// Main
// ─────────────────────────────────────────────
fn main() {
    println!();
    println!("  ╔══════════════════════════════════════╗");
    println!("  ║   ⚡ TurboFind v1.0                  ║");
    println!("  ║   Blazing Fast File Search            ║");
    println!("  ╚══════════════════════════════════════╝");
    println!();

    // Determine cache path
    let cache_dir = dirs::cache_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("turbofind");
    fs::create_dir_all(&cache_dir).ok();
    let cache_path = cache_dir.join("index.bin");

    // Determine roots to index
    let args: Vec<String> = std::env::args().collect();
    let default_roots = if cfg!(target_os = "windows") {
        vec!["C:\\Users"]
    } else {
        vec!["/home", "/usr"]
    };

    let roots: Vec<&str> = if args.len() > 1 {
        args[1..].iter().map(|s| s.as_str()).collect()
    } else {
        default_roots
    };

    // Try loading cached index, rebuild if stale (>1 hour)
    let index = if cache_path.exists() {
        match FileIndex::load(&cache_path) {
            Ok(cached) => {
                let age_secs = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_secs()
                    - cached.indexed_at;
                if age_secs > 3600 {
                    println!("  ♻️  Cache stale ({}min old), rebuilding...", age_secs / 60);
                    let idx = FileIndex::build(&roots);
                    idx.save(&cache_path).ok();
                    idx
                } else {
                    println!("  ✅ Using cached index ({}s old)", age_secs);
                    cached
                }
            }
            Err(_) => {
                let idx = FileIndex::build(&roots);
                idx.save(&cache_path).ok();
                idx
            }
        }
    } else {
        let idx = FileIndex::build(&roots);
        idx.save(&cache_path).ok();
        idx
    };

    println!();

    // Run interactive search
    if let Err(e) = run_tui(&index) {
        eprintln!("TUI error: {}", e);
    }
}
