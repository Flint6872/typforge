#![cfg(feature = "intel")]
use std::fs;
use std::path::{Path, PathBuf};
use typst::syntax::Side;
use typst::syntax::Source;
pub use typst_ide::{Completion, IdeWorld, Tooltip, autocomplete, tooltip};
use typst_layout::PagedDocument;

/// A portable, serializable classification for completion options.
/// Useful for other editors (Python, TypeScript/WASM, terminal) to map cleanly to their own UI.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub enum CompletionKind {
    Func,
    Type,
    Param,
    Constant,
    Symbol,
    Unit,    // For coaching suggestions
    Package, // Autocompletion for installed local/cached packages
    Text,
}

/// A clean, pre-processed completion payload ready for editor consumption.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct TypastryCompletion {
    pub label: String,
    pub apply: String,
    pub detail: Option<String>,
    pub kind: CompletionKind,
}

/// Represents an on-disk package found during completion scans
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiscoveredPackage {
    pub namespace: String,
    pub name: String,
    pub version: String,
}

/// Scans local data and cache directories for installed/cached packages
pub fn discover_installed_packages() -> Vec<DiscoveredPackage> {
    let mut packages = Vec::new();

    let scan_roots = [
        typst_kit::packages::FsPackages::system_data().map(|p| p.path().to_path_buf()),
        typst_kit::packages::FsPackages::system_cache().map(|p| p.path().to_path_buf()),
    ];

    for root_opt in scan_roots.into_iter().flatten() {
        if !root_opt.exists() {
            continue;
        }

        // 1. Scan namespaces (e.g. preview, local)
        if let Ok(ns_entries) = fs::read_dir(&root_opt) {
            for ns_entry in ns_entries.flatten() {
                if ns_entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                    let ns_name = ns_entry.file_name().to_string_lossy().to_string();
                    let ns_path = ns_entry.path();

                    // 2. Scan package names
                    if let Ok(pkg_entries) = fs::read_dir(&ns_path) {
                        for pkg_entry in pkg_entries.flatten() {
                            if pkg_entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                                let pkg_name = pkg_entry.file_name().to_string_lossy().to_string();
                                let pkg_path = pkg_entry.path();

                                // 3. Scan versions
                                if let Ok(ver_entries) = fs::read_dir(&pkg_path) {
                                    for ver_entry in ver_entries.flatten() {
                                        if ver_entry
                                            .file_type()
                                            .map(|t| t.is_dir())
                                            .unwrap_or(false)
                                        {
                                            let ver_name =
                                                ver_entry.file_name().to_string_lossy().to_string();
                                            let ver_path = ver_entry.path();

                                            // Basic validity check: contains typst.toml or at least some content
                                            let has_manifest = ver_path.join("typst.toml").exists();
                                            let has_files = fs::read_dir(&ver_path)
                                                .map(|mut i| i.next().is_some())
                                                .unwrap_or(false);

                                            if has_manifest || has_files {
                                                let item = DiscoveredPackage {
                                                    namespace: ns_name.clone(),
                                                    name: pkg_name.clone(),
                                                    version: ver_name,
                                                };
                                                if !packages.contains(&item) {
                                                    packages.push(item);
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    packages
}

/// Retrieves raw, context-aware completions from `typst-ide`.
pub fn get_completions(
    world: &dyn IdeWorld,
    document: Option<&PagedDocument>,
    source: &Source,
    cursor_index: usize,
    explicit: bool,
) -> Vec<Completion> {
    autocomplete(world, document, source, cursor_index, explicit)
        .map(|(_, completions)| completions)
        .unwrap_or_default()
}

/// Retrieves tooltip/documentation at the specified cursor position.
pub fn get_hover_info(
    world: &dyn IdeWorld,
    document: Option<&PagedDocument>,
    source: &Source,
    cursor_index: usize,
) -> Option<Tooltip> {
    tooltip(world, document, source, cursor_index, Side::After)
}

/// Checks if the cursor is inside a package import string like `#import "@..."`
/// Returns `Some(query_text)` (the text between the opening quote and cursor).
pub fn detect_package_import_context(content: &str, cursor_index: usize) -> Option<String> {
    if cursor_index == 0 || cursor_index > content.len() {
        return None;
    }

    let before_cursor = &content[..cursor_index];

    // Find the nearest opening quote `"` before cursor on this line
    let last_quote_idx = before_cursor.rfind('"')?;
    let text_inside_quotes = &before_cursor[last_quote_idx + 1..];

    // It must start with '@' to qualify as a package import
    if !text_inside_quotes.starts_with('@') {
        return None;
    }

    // Ensure this quote is preceded by `import` or `#import`
    let preceding_text = before_cursor[..last_quote_idx].trim_end();
    if preceding_text.ends_with("import") {
        Some(text_inside_quotes.to_string())
    } else {
        None
    }
}

/// Generates package autocompletions based on installed/cached packages on disk.
fn get_package_completions(query: &str) -> Vec<TypastryCompletion> {
    let installed = discover_installed_packages();
    let mut completions = Vec::new();

    // Query format variants:
    // 1. "@" -> list all installed packages
    // 2. "@preview/" -> list packages in that namespace
    // 3. "@preview/cetz:" -> list installed versions for that package
    let query_lower = query.to_lowercase();

    for pkg in installed {
        let full_spec = format!("@{}/{}:{}", pkg.namespace, pkg.name, pkg.version);
        let full_spec_lower = full_spec.to_lowercase();

        if full_spec_lower.starts_with(&query_lower) {
            completions.push(TypastryCompletion {
                label: full_spec.clone(),
                apply: full_spec.clone(),
                detail: Some(format!(
                    "Package: {} (v{}) [{}]",
                    pkg.name, pkg.version, pkg.namespace
                )),
                kind: CompletionKind::Package,
            });
        }
    }

    // Sort alphabetically by package label
    completions.sort_by(|a, b| a.label.cmp(&b.label));
    completions
}

/// High-level, modular completions engine. Fetches, filters, and applies
/// package completions and coaching rules on pure Rust primitives.
pub fn get_enhanced_completions(
    world: &dyn IdeWorld,
    document: Option<&PagedDocument>,
    source: &Source,
    cursor_index: usize,
) -> Vec<TypastryCompletion> {
    let content = source.text();

    // 1. --- PHASE 2: PACKAGE IMPORT COMPLETIONS ---
    // If the cursor is typing inside `#import "@..."`, return package completions directly
    if let Some(query) = detect_package_import_context(content, cursor_index) {
        let package_completions = get_package_completions(&query);
        if !package_completions.is_empty() {
            return package_completions;
        }
    }

    let mut results = Vec::new();

    // 2. Fetch raw underlying compiler suggestion list
    let raw_completions = get_completions(world, document, source, cursor_index, false);

    // 3. Discover cursor contexts
    let (trigger_offset, is_hash_command) = get_trigger_info(content, cursor_index);
    let typed_prefix = if cursor_index > trigger_offset {
        let start = if is_hash_command {
            trigger_offset + 1
        } else {
            trigger_offset
        };
        if cursor_index > start {
            content
                .chars()
                .skip(start)
                .take(cursor_index - start)
                .collect::<String>()
                .to_lowercase()
        } else {
            String::new()
        }
    } else {
        String::new()
    };

    // 4. Process & filter compiler suggestions
    let filtered: Vec<TypastryCompletion> = raw_completions
        .into_iter()
        .filter(|c| {
            if typed_prefix.is_empty() {
                true
            } else {
                c.label
                    .to_string()
                    .to_lowercase()
                    .starts_with(&typed_prefix)
            }
        })
        .map(|c| {
            let label = c.label.to_string();
            let raw_apply_text = c
                .apply
                .as_ref()
                .map(|s| s.to_string())
                .unwrap_or_else(|| label.clone());

            // Handle import completions: insert `import "@` so cursor lands on `@`
            let apply_text =
                if label.starts_with("import") || raw_apply_text.starts_with("import \"@") {
                    "import \"@".to_string()
                } else if raw_apply_text.contains('(') {
                    let base = raw_apply_text.split('(').next().unwrap_or(&label);
                    format!("{}()", base)
                } else {
                    raw_apply_text
                        .replace("${}", "")
                        .replace("${1:}", "")
                        .replace("${2:}", "")
                };

            let kind = match c.kind {
                typst_ide::CompletionKind::Func => CompletionKind::Func,
                typst_ide::CompletionKind::Type => CompletionKind::Type,
                typst_ide::CompletionKind::Param => CompletionKind::Param,
                typst_ide::CompletionKind::Constant => CompletionKind::Constant,
                typst_ide::CompletionKind::Package => CompletionKind::Package,
                typst_ide::CompletionKind::Symbol(_) => CompletionKind::Symbol,
                _ => CompletionKind::Text,
            };

            TypastryCompletion {
                label,
                apply: apply_text,
                detail: c.detail.map(|s| s.to_string()),
                kind,
            }
        })
        .collect();

    results.extend(filtered);

    // 5. --- COACHING: PHYSICAL DIMENSIONS & UNITS HEURISTICS ---
    if !typed_prefix.is_empty() && is_physical_dimension_context(content, trigger_offset) {
        if let Ok(_number) = typed_prefix.parse::<f64>() {
            let units = ["pt", "em", "cm", "mm"];
            for unit in units {
                let suggested_text = format!("{}{}", typed_prefix, unit);
                results.insert(
                    0, // Push coaching recommendations directly to the top!
                    TypastryCompletion {
                        label: suggested_text.clone(),
                        apply: suggested_text,
                        detail: Some(format!("Coaching: Insert explicit length ({})", unit)),
                        kind: CompletionKind::Unit,
                    },
                );
            }
        }
    }

    results
}

/// Helper function to detect if the target parameter context expects physical dimensions / units.
pub fn is_physical_dimension_context(content: &str, mut offset: usize) -> bool {
    let chars: Vec<char> = content.chars().collect();

    // Skip spaces backwards
    while offset > 0 {
        if let Some(c) = chars.get(offset - 1) {
            if c.is_whitespace() {
                offset -= 1;
                continue;
            }
        }
        break;
    }

    // Check for parameter assignment separator `:`
    if offset == 0 || chars.get(offset - 1) != Some(&':') {
        return false;
    }
    offset -= 1;

    // Skip spaces backwards
    while offset > 0 {
        if let Some(c) = chars.get(offset - 1) {
            if c.is_whitespace() {
                offset -= 1;
                continue;
            }
        }
        break;
    }

    // Scan backward to extract parameter name identifier
    let mut param_start = offset;
    while param_start > 0 {
        if let Some(prev_c) = chars.get(param_start - 1) {
            if prev_c.is_alphanumeric() || *prev_c == '_' || *prev_c == '-' {
                param_start -= 1;
                continue;
            }
        }
        break;
    }

    if param_start < offset {
        let param_name: String = chars[param_start..offset].iter().collect();
        matches!(
            param_name.as_str(),
            "size"
                | "margin"
                | "gap"
                | "gutter"
                | "width"
                | "height"
                | "radius"
                | "stroke"
                | "inset"
                | "outset"
                | "spacing"
        )
    } else {
        false
    }
}

/// Scans backwards in a source string to find the trigger boundary.
/// Returns: `(trigger_byte_offset, is_hash_command)`
pub fn get_trigger_info(content: &str, cursor: usize) -> (usize, bool) {
    if cursor == 0 {
        return (0, false);
    }
    let chars: Vec<char> = content.chars().collect();

    // 1. Find the start of the current alphanumeric word/identifier (allowing decimals)
    let mut word_start = cursor;
    while word_start > 0 {
        if let Some(prev_c) = chars.get(word_start - 1) {
            if prev_c.is_alphanumeric() || *prev_c == '_' || *prev_c == '-' || *prev_c == '.' {
                word_start -= 1;
                continue;
            }
        }
        break;
    }

    // 2. Check if the character immediately preceding the word is '#'
    if word_start > 0 && chars.get(word_start - 1) == Some(&'#') {
        return (word_start - 1, true);
    }

    (word_start, false)
}
