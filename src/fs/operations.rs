use std::fs::{self, File};
use std::io::{BufRead, BufReader, Write};
use std::path::Path;
use globset::GlobBuilder;
use regex::RegexBuilder;
use serde::{Deserialize, Serialize};
use similar::TextDiff;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EditResult {
    pub message: String,
    pub path: String,
    pub old_text: String,
    pub new_text: String,
    pub replacements_count: usize,
    pub unified_diff: String,
}

impl std::fmt::Display for EditResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.unified_diff.is_empty() {
            write!(f, "{}", self.message)
        } else {
            write!(f, "{}\n\n{}", self.message, self.unified_diff)
        }
    }
}

/// Generates a standard unified diff between two text contents.
pub fn generate_unified_diff(path: &str, old_content: &str, new_content: &str) -> String {
    let diff = TextDiff::from_lines(old_content, new_content);
    diff.unified_diff()
        .context_radius(3)
        .header(&format!("a/{}", path), &format!("b/{}", path))
        .to_string()
}

/// Read a text file with line numbers, offset, and limit.
pub fn read_file(
    path_str: &str,
    offset: Option<usize>,
    limit: Option<usize>,
    max_bytes: Option<usize>,
) -> Result<String, String> {
    let path = Path::new(path_str);
    if !path.exists() {
        return Err(format!("File '{}' not found", path_str));
    }
    if path.is_dir() {
        return Err(format!("Path '{}' is a directory, not a file", path_str));
    }

    let file = File::open(path).map_err(|e| format!("Failed to open '{}': {}", path_str, e))?;
    let reader = BufReader::new(file);

    let start_line = offset.unwrap_or(1).max(1);
    let max_lines = limit.unwrap_or(2000);
    let byte_cap = max_bytes.unwrap_or(131072); // 128 KB default

    let mut out = String::new();
    let mut lines_iter = reader.lines();
    let mut lines_read = 0;
    let mut current_line = 0;
    let mut truncated_by_bytes = false;
    let mut has_more_lines = false;

    while let Some(line_result) = lines_iter.next() {
        current_line += 1;
        if current_line < start_line {
            continue;
        }

        let line = line_result.map_err(|e| format!("Error reading line {}: {}", current_line, e))?;
        let entry = format!("{}:{}\n", current_line, line);

        if out.len() + entry.len() > byte_cap {
            truncated_by_bytes = true;
            break;
        }

        out.push_str(&entry);
        lines_read += 1;

        if lines_read >= max_lines {
            if lines_iter.next().is_some() {
                has_more_lines = true;
            }
            break;
        }
    }

    if truncated_by_bytes {
        out.push_str(&format!("[truncated: max_bytes cap reached at line {}]\n", current_line));
    } else if has_more_lines {
        out.push_str(&format!("[more lines: next offset={}]\n", current_line + 1));
    }

    Ok(out)
}

/// Exact substring replacement with atomic write and unified diff generation.
pub fn edit_file(
    path_str: &str,
    old_string: &str,
    new_string: &str,
    replace_all: bool,
) -> Result<EditResult, String> {
    let path = Path::new(path_str);
    if !path.exists() {
        return Err(format!("File '{}' not found", path_str));
    }
    if path.is_dir() {
        return Err(format!("Path '{}' is a directory, not a file", path_str));
    }

    let content = fs::read_to_string(path).map_err(|e| format!("Failed to read '{}': {}", path_str, e))?;
    let count = content.matches(old_string).count();

    if count == 0 {
        return Err(format!("old_string not found in '{}'", path_str));
    }

    if !replace_all && count > 1 {
        return Err(format!(
            "old_string matched {} times in '{}'. Set replace_all=true or provide unique surrounding context.",
            count, path_str
        ));
    }

    let new_content = if replace_all {
        content.replace(old_string, new_string)
    } else {
        content.replacen(old_string, new_string, 1)
    };

    atomic_write(path, new_content.as_bytes())?;

    let unified_diff = generate_unified_diff(path_str, &content, &new_content);
    let s = if count > 1 { "es" } else { "" };
    let message = format!("The file '{}' has been updated successfully ({} match{} replaced).", path_str, count, s);

    Ok(EditResult {
        message,
        path: path_str.to_string(),
        old_text: old_string.to_string(),
        new_text: new_string.to_string(),
        replacements_count: count,
        unified_diff,
    })
}

/// Direct file write with atomic safety and directory auto-creation.
pub fn write_file(
    path_str: &str,
    content: &str,
    create_dirs: bool,
) -> Result<String, String> {
    let path = Path::new(path_str);
    if create_dirs {
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() && !parent.exists() {
                fs::create_dir_all(parent)
                    .map_err(|e| format!("Failed to create directories for '{}': {}", path_str, e))?;
            }
        }
    }

    atomic_write(path, content.as_bytes())?;
    Ok(format!("OK: wrote {} bytes to {}", content.len(), path_str))
}

/// Unified file and content search (glob + grep) respecting .gitignore.
pub fn search_files(
    path_str: &str,
    pattern: &str,
    glob_pattern: Option<&str>,
    max_results: Option<usize>,
    case_sensitive: Option<bool>,
) -> Result<String, String> {
    let regex = if !pattern.is_empty() {
        Some(
            RegexBuilder::new(pattern)
                .case_insensitive(!case_sensitive.unwrap_or(true))
                .build()
                .map_err(|e| format!("Invalid regex pattern '{}': {}", pattern, e))?,
        )
    } else {
        None
    };

    let glob_matcher = if let Some(gp) = glob_pattern {
        let g = GlobBuilder::new(gp)
            .literal_separator(false)
            .build()
            .map_err(|e| format!("Invalid glob pattern '{}': {}", gp, e))?;
        Some(g.compile_matcher())
    } else {
        None
    };

    let limit = max_results.unwrap_or(250);
    let mut out = String::new();
    let mut match_count = 0;
    let mut truncated = false;

    let walker = ignore::WalkBuilder::new(path_str)
        .hidden(true)
        .parents(true)
        .git_ignore(true)
        .git_global(true)
        .git_exclude(true)
        .build();

    for result in walker {
        let entry = match result {
            Ok(e) => e,
            Err(_) => continue,
        };

        if !entry.file_type().map(|ft| ft.is_file()).unwrap_or(false) {
            continue;
        }

        let entry_path = entry.path();
        if let Some(ref matcher) = glob_matcher {
            let matches_path = matcher.is_match(entry_path);
            let matches_filename = entry_path.file_name().map(|n| matcher.is_match(n)).unwrap_or(false);
            if !matches_path && !matches_filename {
                continue;
            }
        }

        // If pattern is empty, this acts as a glob file finder
        if regex.is_none() {
            match_count += 1;
            out.push_str(&format!("{}\n", entry_path.display()));
            if match_count >= limit {
                truncated = true;
                break;
            }
            continue;
        }

        let re = regex.as_ref().unwrap();
        let file = match File::open(entry_path) {
            Ok(f) => f,
            Err(_) => continue,
        };

        let reader = BufReader::new(file);
        let mut line_no = 0;

        for line_res in reader.lines() {
            line_no += 1;
            let line = match line_res {
                Ok(l) => l,
                Err(_) => break, // Skip binary or non-UTF8 files cleanly
            };

            if re.is_match(&line) {
                match_count += 1;
                out.push_str(&format!("{}:{}:{}\n", entry_path.display(), line_no, line));
                if match_count >= limit {
                    truncated = true;
                    break;
                }
            }
        }

        if truncated {
            break;
        }
    }

    if match_count == 0 {
        return Ok("No matches found.".to_string());
    }

    if truncated {
        out.push_str(&format!("[truncated: reached limit of {} matches]\n", limit));
    }

    Ok(out)
}

fn atomic_write(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    let temp_name = format!(
        ".tmp_{}_{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos()
    );
    let temp_path = parent.join(temp_name);

    let mut temp_file = File::create(&temp_path)
        .map_err(|e| format!("Failed to create temporary file for '{}': {}", path.display(), e))?;

    if let Err(e) = temp_file.write_all(bytes) {
        let _ = fs::remove_file(&temp_path);
        return Err(format!("Failed to write to '{}': {}", path.display(), e));
    }

    if let Err(e) = temp_file.sync_all() {
        let _ = fs::remove_file(&temp_path);
        return Err(format!("Failed to sync '{}': {}", path.display(), e));
    }

    drop(temp_file);

    if let Err(e) = fs::rename(&temp_path, path) {
        // Fallback for Windows or cross-filesystem moves
        if let Err(write_err) = fs::write(path, bytes) {
            let _ = fs::remove_file(&temp_path);
            return Err(format!(
                "Failed to rename temporary file or write '{}': rename={}, write={}",
                path.display(), e, write_err
            ));
        }
        let _ = fs::remove_file(&temp_path);
    }

    Ok(())
}
