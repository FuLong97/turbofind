// TurboFind - Fast File Indexer for Windows
// Crawls filesystem in parallel, caches index to disk, fuzzy searches with nucleo

use crossterm::{
    cursor,
    event::{self, Event, KeyCode, KeyEvent, KeyModifiers},
    execute, terminal,
};
use nucleo::pattern::{CaseMatching, Normalization, Pattern};
use nucleo::{Matcher, Utf32Str};
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::time::{Instant, SystemTime, UNIX_EPOCH};
use walkdir::WalkDir;

/// Single indexed file/directory entry
#[derive(Serialize, Deserialize, Clone, Debug)]
struct FileEntry {
    path: String,
    name: String,
    name_lower: String, // pre-computed for case-insensitive matching
    size: u64,
    is_dir: bool,
    extension: String, // lowercase, no dot
    modified: u64,     // unix timestamp
}

/// In-memory search index with extension lookup table
#[derive(Serialize, Deserialize)]
struct FileIndex {
    entries: Vec<FileEntry>,
    ext_map: HashMap<String, Vec<usize>>, // extension -> indices into entries
    indexed_at: u64,                      // when this index was built
    roots: Vec<String>,                   // which paths were indexed
}

impl FileIndex {
    /// Crawl filesystem roots in parallel and build the index
    fn build(roots: &[&str]) -> Self {
        let start = Instant::now();
        println!("  Indexing filesystem...");

        // rayon parallelizes across roots, walkdir handles recursion
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

                    // skip hidden and system directories
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

        // build extension -> index lookup for O(1) ext: filtering
        let mut ext_map: HashMap<String, Vec<usize>> = HashMap::new();
        for (i, entry) in all_entries.iter().enumerate() {
            if !entry.extension.is_empty() {
                ext_map.entry(entry.extension.clone()).or_default().push(i);
            }
        }

        let count = all_entries.len();
        let elapsed = start.elapsed();
        println!(
            "  Indexed {} files in {:.2}s ({:.0} files/sec)",
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

    /// Serialize index to binary file (bincode) for fast reload
    fn save(&self, path: &Path) -> io::Result<()> {
        let data = bincode::serialize(self).map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
        fs::write(path, data)
    }

    /// Deserialize index from binary cache
    fn load(path: &Path) -> io::Result<Self> {
        let data = fs::read(path)?;
        let index: Self =
            bincode::deserialize(&data).map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
        println!("  Loaded index: {} files from cache", index.entries.len());
        Ok(index)
    }

    /// Fuzzy search with optional filters (ext:, dir:)
    /// Uses nucleo matcher with parallel scoring via rayon's map_init
    /// Exact substring matches get a +10000 score boost
    fn search(&self, query: &str, max_results: usize) -> Vec<(&FileEntry, i64)> {
        // parse query into filters and search terms
        // e.g. "ext:rs config" -> ext_filter="rs", search_query="config"
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
        let search_lower = search_query.to_lowercase(); // compute once, not per entry
        let pattern = Pattern::parse(&search_query, CaseMatching::Ignore, Normalization::Smart);

        // map_init creates a Matcher + buffer per thread (Matcher isn't Send)
        // each thread scores its chunk of entries independently
        let mut results: Vec<(&FileEntry, i64)> = self
            .entries
            .par_iter()
            .map_init(
                || (Matcher::new(nucleo::Config::DEFAULT), Vec::new()),
                |(matcher, buf), entry| {
                    // apply filters before expensive fuzzy matching
                    if let Some(ref ext) = ext_filter {
                        if &entry.extension != ext {
                            return None;
                        }
                    }
                    if dir_only && !entry.is_dir {
                        return None;
                    }
                    if search_query.is_empty() {
                        return Some((entry, 0i64));
                    }

                    // nucleo fuzzy score — None means no match
                    pattern
                        .score(Utf32Str::new(&entry.name_lower, buf), matcher)
                        .map(|score| {
                            let mut s = score as i64;
                            // boost files that contain the exact query as substring
                            // so "steam.exe" ranks above "ms-teams.exe" for query "steam"
                            if entry.name_lower.contains(&search_lower) {
                                s += 10000;
                            }
                            (entry, s)
                        })
                },
            )
            .flatten() // remove None values from filter_map
            .collect();

        // sort by score descending, truncate to max_results
        results.sort_by(|a, b| b.1.cmp(&a.1));
        results.truncate(max_results);
        results
    }
}

/// Pad or truncate string to exactly w chars.
/// Ensures every TUI line is the same width — prevents flicker and line wrapping.
fn fit(s: &str, w: usize) -> String {
    if s.len() >= w {
        s[..w].to_string()
    } else {
        format!("{:<width$}", s, width = w)
    }
}

/// Human-readable file size (compact format)
fn format_size(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;
    if bytes >= GB {
        format!("{:.1}G", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.1}M", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{}K", bytes / KB)
    } else {
        format!("{}B", bytes)
    }
}

/// Interactive TUI — renders entire frame into a buffer, writes once to avoid flicker
fn run_tui(index: &FileIndex) -> io::Result<()> {
    let mut stdout = io::stdout();
    terminal::enable_raw_mode()?;
    execute!(stdout, terminal::EnterAlternateScreen, cursor::Hide)?;

    let mut query = String::new();
    let mut results: Vec<(&FileEntry, i64)> = Vec::new();
    let mut selected: usize = 0;
    let mut search_time = std::time::Duration::ZERO;
    let mut buf = String::with_capacity(16384); // pre-alloc frame buffer

    loop {
        let (cols, rows) = terminal::size().unwrap_or((80, 24));
        let w = cols as usize;
        let max_results = (rows as usize).saturating_sub(6); // reserve rows for header/footer

        // build entire frame into buf, then write once = zero flicker
        buf.clear();

        // row 0: header with file count and search stats
        let header = if !query.is_empty() {
            format!(
                "  TurboFind | {} files | {} results in {:.1}ms",
                index.entries.len(),
                results.len(),
                search_time.as_secs_f64() * 1000.0
            )
        } else {
            format!("  TurboFind | {} files indexed", index.entries.len())
        };
        buf.push_str(&fit(&header, w));

        // row 1: spacer
        buf.push_str(&fit("", w));

        // row 2: search input
        let search_line = if query.is_empty() {
            "  > Type to search... (ext:rs for filters, Esc to quit)".to_string()
        } else {
            format!("  > {}", query)
        };
        buf.push_str(&fit(&search_line, w));

        // row 3: separator
        buf.push_str(&fit(&"-".repeat(w), w));

        // rows 4+: search results
        for i in 0..max_results {
            if let Some((entry, _)) = results.get(i) {
                // file type tag for quick visual identification
                let tag = if entry.is_dir {
                    "DIR"
                } else {
                    match entry.extension.as_str() {
                        "rs" | "py" | "js" | "ts" | "c" | "cpp" | "java" | "go" => "SRC",
                        "jpg" | "png" | "gif" | "bmp" | "svg" | "webp" => "IMG",
                        "mp3" | "wav" | "flac" | "ogg" | "m4a" => "AUD",
                        "mp4" | "mkv" | "avi" | "mov" | "webm" => "VID",
                        "zip" | "rar" | "7z" | "tar" | "gz" => "ZIP",
                        "exe" | "msi" => "EXE",
                        "pdf" => "PDF",
                        "doc" | "docx" => "DOC",
                        "xls" | "xlsx" => "XLS",
                        _ => "   ",
                    }
                };

                // show last 2 path components to keep it readable
                let parent = Path::new(&entry.path)
                    .parent()
                    .map(|p| p.to_string_lossy().to_string())
                    .unwrap_or_default();
                let short_path: String = Path::new(&parent)
                    .components()
                    .rev()
                    .take(2)
                    .collect::<Vec<_>>()
                    .into_iter()
                    .rev()
                    .map(|c| c.as_os_str().to_string_lossy().to_string())
                    .collect::<Vec<_>>()
                    .join("\\");

                let size_str = if entry.is_dir {
                    String::new()
                } else {
                    format_size(entry.size)
                };
                let marker = if i == selected { ">" } else { " " };
                let line = format!(
                    " {} [{}] {}  {}  {}",
                    marker, tag, entry.name, short_path, size_str
                );
                buf.push_str(&fit(&line, w));
            } else {
                // blank line to overwrite stale results
                buf.push_str(&fit("", w));
            }
        }

        // single write of the entire frame buffer
        execute!(stdout, cursor::MoveTo(0, 0))?;
        write!(stdout, "{}", buf)?;

        // footer with keybindings
        execute!(stdout, cursor::MoveTo(0, rows - 1))?;
        write!(
            stdout,
            "{}",
            fit(
                "  Up/Down: Navigate | Enter: Open | Ctrl+O: Folder | Ctrl+B: Bench | Esc: Quit",
                w
            )
        )?;

        stdout.flush()?;

        // --- input handling ---
        // only process KeyPress events (Windows sends Press + Release)
        if let Event::Key(key) = event::read()? {
            if key.kind != event::KeyEventKind::Press {
                continue;
            }
            match key {
                // exit
                KeyEvent {
                    code: KeyCode::Esc, ..
                } => break,
                KeyEvent {
                    code: KeyCode::Char('c'),
                    modifiers: KeyModifiers::CONTROL,
                    ..
                } => break,

                // delete last character
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

                // open containing folder in explorer
                KeyEvent {
                    code: KeyCode::Char('o'),
                    modifiers: KeyModifiers::CONTROL,
                    ..
                } => {
                    if let Some((entry, _)) = results.get(selected) {
                        let folder = Path::new(&entry.path)
                            .parent()
                            .map(|p| p.to_string_lossy().to_string())
                            .unwrap_or_default();
                        #[cfg(target_os = "windows")]
                        {
                            let _ = std::process::Command::new("explorer").arg(&folder).spawn();
                        }
                        #[cfg(target_os = "linux")]
                        {
                            let _ = std::process::Command::new("xdg-open").arg(&folder).spawn();
                        }
                    }
                }

                // run benchmark (leaves TUI temporarily)
                KeyEvent {
                    code: KeyCode::Char('b'),
                    modifiers: KeyModifiers::CONTROL,
                    ..
                } => {
                    // exit alternate screen to show benchmark output
                    execute!(stdout, terminal::LeaveAlternateScreen, cursor::Show)?;
                    terminal::disable_raw_mode()?;

                    let test_queries = ["main", "config", "test", "cargo", "readme"];
                    println!("\n  Running benchmark...");
                    for q in &test_queries {
                        let start = Instant::now();
                        let iterations = 100;
                        for _ in 0..iterations {
                            let _ = index.search(q, 100);
                        }
                        let avg = start.elapsed() / iterations;
                        println!("    '{}' -> avg {:.2}ms", q, avg.as_secs_f64() * 1000.0);
                    }
                    println!("  Press Enter to continue...");
                    let _ = io::stdin().read_line(&mut String::new());

                    // re-enter TUI
                    terminal::enable_raw_mode()?;
                    execute!(stdout, terminal::EnterAlternateScreen, cursor::Hide)?;
                }

                // type a character -> update search
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

                // navigate results
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

                // open selected file with default application
                KeyEvent {
                    code: KeyCode::Enter,
                    ..
                } => {
                    if let Some((entry, _)) = results.get(selected) {
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
                            let _ = std::process::Command::new("open").arg(&entry.path).spawn();
                        }
                    }
                }
                _ => {}
            }
        }
    }

    // restore terminal state
    execute!(stdout, terminal::LeaveAlternateScreen, cursor::Show)?;
    terminal::disable_raw_mode()?;
    Ok(())
}

fn main() {
    // cache lives in %LOCALAPPDATA%/turbofind/index.bin (Windows)
    let cache_dir = dirs::cache_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("turbofind");
    fs::create_dir_all(&cache_dir).ok();
    let cache_path = cache_dir.join("index.bin");

    // allow custom roots via CLI args, otherwise use defaults
    let args: Vec<String> = std::env::args().collect();
    let default_roots = if cfg!(target_os = "windows") {
        vec!["C:\\Users", "C:\\Program Files", "C:\\Program Files (x86)"]
    } else {
        vec!["/home", "/usr"]
    };

    let roots: Vec<&str> = if args.len() > 1 {
        args[1..].iter().map(|s| s.as_str()).collect()
    } else {
        default_roots
    };

    // load cached index if fresh (<1 hour), otherwise rebuild
    let index = if cache_path.exists() {
        match FileIndex::load(&cache_path) {
            Ok(cached) => {
                let age = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_secs()
                    - cached.indexed_at;
                if age > 3600 {
                    println!("  Cache stale, rebuilding...");
                    let idx = FileIndex::build(&roots);
                    idx.save(&cache_path).ok();
                    idx
                } else {
                    println!("  Using cached index ({}s old)", age);
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

    if let Err(e) = run_tui(&index) {
        eprintln!("TUI error: {}", e);
    }
}
