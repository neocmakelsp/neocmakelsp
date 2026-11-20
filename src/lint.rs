use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Result, bail};
use ignore::Walk;
use tower_lsp::lsp_types::{Diagnostic, Message};

use crate::config::CONFIG;
use crate::grammar::{LintConfigInfo, checkerror};

/// Lint CMake files and print their diagnostics.
///
/// Returns `true` when at least one diagnostic was reported.
pub fn lint(paths: &[PathBuf]) -> Result<bool> {
    let mut files = collect_cmake_files(paths)?;
    files.sort();
    files.dedup();

    let mut has_diagnostics = false;
    for file in &files {
        let source = match fs::read_to_string(file) {
            Ok(source) => source,
            Err(error) => {
                tracing::warn!("Failed to read '{}': {error}", file.display());
                has_diagnostics = true;
                continue;
            }
        };

        let Some(mut diagnostics) = checkerror(
            file,
            &source,
            LintConfigInfo {
                use_lint: true,
                use_extra_cmake_lint: CONFIG.enable_external_cmake_lint,
            },
        ) else {
            continue;
        };
        diagnostics.sort_by_key(|diagnostic| {
            (
                diagnostic.range.start.line,
                diagnostic.range.start.character,
            )
        });
        for diagnostic in &diagnostics {
            print_diagnostic(file, diagnostic);
        }
        has_diagnostics = true;
    }

    Ok(has_diagnostics)
}

fn collect_cmake_files(paths: &[PathBuf]) -> Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    for path in paths {
        if path.is_file() {
            files.push(path.clone());
        } else if path.is_dir() {
            for entry in Walk::new(path).flatten() {
                let entry_path = entry.path();
                if entry_path.is_file() && is_cmake_file(entry_path) {
                    files.push(entry_path.to_path_buf());
                }
            }
        } else {
            bail!("Path '{}' does not exist", path.display());
        }
    }
    Ok(files)
}

fn is_cmake_file(path: &Path) -> bool {
    path.file_name()
        .is_some_and(|name| name == "CMakeLists.txt")
        || path
            .extension()
            .is_some_and(|extension| extension == "cmake")
}

fn print_diagnostic(path: &Path, diagnostic: &Diagnostic) {
    let message = match &diagnostic.message {
        Message::String(message) => message.as_str(),
        Message::MarkupContent(content) => content.value.as_str(),
    };
    println!(
        "{}:{}:{}: {}",
        display_path(path),
        diagnostic.range.start.line + 1,
        diagnostic.range.start.character + 1,
        message,
    );
}

fn display_path(path: &Path) -> String {
    let path = path.strip_prefix(".").unwrap_or(path);
    let Ok(current_dir) = std::env::current_dir() else {
        return path.display().to_string();
    };
    pathdiff::diff_paths(path, current_dir)
        .filter(|relative| !relative.starts_with(".."))
        .unwrap_or_else(|| path.to_path_buf())
        .display()
        .to_string()
}
