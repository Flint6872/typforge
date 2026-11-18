#![cfg(feature = "intel")]
use std::fs;
use std::ops::Range;
use typst::syntax::{Side, Source};
pub use typst_ide::{Completion, IdeWorld, Tooltip, autocomplete, tooltip};
use typst_layout::PagedDocument;
use typst_syntax::{LinkedNode, SyntaxKind, ast, ast::AstNode};

/// A completion payload with an exact byte replacement range in the source.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct CompletionResponse {
    pub replace_range: Range<usize>,
    pub items: Vec<TypastryCompletion>,
}

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
/// using the `typst-syntax` CST.
/// Returns `Some((query_text, replace_range))` where `replace_range` covers the
/// text between the opening `"` and `cursor_index`.
pub fn detect_package_import_context_ast(
    source: &Source,
    cursor_index: usize,
) -> Option<(String, Range<usize>)> {
    let content = source.text();
    if cursor_index == 0 || cursor_index > content.len() || !content.is_char_boundary(cursor_index)
    {
        return None;
    }

    let root = LinkedNode::new(source.root());
    // Use or_else to robustly find a leaf node at or around the cursor
    let Some(leaf) = root
        .leaf_at(cursor_index, Side::Before)
        .or_else(|| root.leaf_at(cursor_index, Side::After))
    else {
        return None;
    };

    if !matches!(leaf.kind(), SyntaxKind::Str | SyntaxKind::Error) {
        return None;
    }

    let leaf_range = leaf.range();
    let leaf_text = &content[leaf_range.clone()];
    if !leaf_text.starts_with('"') || cursor_index <= leaf_range.start {
        return None;
    }

    // If the string literal is closed, the cursor must sit strictly before the closing quote.
    let is_closed =
        leaf.kind() == SyntaxKind::Str && leaf_text.len() >= 2 && leaf_text.ends_with('"');
    if is_closed && cursor_index >= leaf_range.end {
        return None;
    }

    let query_start = leaf_range.start + 1;
    let text_inside_quotes = &content[query_start..cursor_index];
    if !text_inside_quotes.starts_with('@') {
        return None;
    }

    // Walk ancestors to confirm we are inside a `ModuleImport` node.
    let mut current = leaf.parent().cloned();
    while let Some(node) = current {
        if node.kind() == SyntaxKind::ModuleImport {
            return Some((text_inside_quotes.to_string(), query_start..cursor_index));
        }
        current = node.parent().cloned();
    }

    None
}

/// Backward-compatible wrapper around `detect_package_import_context_ast`.
pub fn detect_package_import_context(content: &str, cursor_index: usize) -> Option<String> {
    let source = Source::detached(content);
    detect_package_import_context_ast(&source, cursor_index).map(|(query, _)| query)
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

/// High-level, modular completions engine returning both the exact replacement
/// byte range and processed completions.
pub fn get_enhanced_completions_with_range(
    world: &dyn IdeWorld,
    document: Option<&PagedDocument>,
    source: &Source,
    cursor_index: usize,
) -> CompletionResponse {
    let content = source.text();

    // 1. Package import completions (`#import "@..."`)
    if let Some((query, replace_range)) = detect_package_import_context_ast(source, cursor_index) {
        let package_completions = get_package_completions(&query);
        if !package_completions.is_empty() {
            return CompletionResponse {
                replace_range,
                items: package_completions,
            };
        }
    }

    let mut results = Vec::new();

    // 2. Fetch raw compiler suggestions (works even when `document` is `None`)
    let raw_completions = get_completions(world, document, source, cursor_index, false);

    // 3. Discover exact AST trigger range & prefix (UTF-8 safe)
    let (replace_range, _is_hash_command) = get_trigger_range_ast(source, cursor_index);
    let typed_prefix = if replace_range.start < replace_range.end
        && content.is_char_boundary(replace_range.start)
        && content.is_char_boundary(replace_range.end)
    {
        content[replace_range.clone()].to_string()
    } else {
        String::new()
    };
    let typed_prefix_lower = typed_prefix.to_lowercase();

    // 4. Process & filter compiler suggestions
    let filtered: Vec<TypastryCompletion> = raw_completions
        .into_iter()
        .filter(|c| {
            if typed_prefix_lower.is_empty() {
                true
            } else {
                c.label
                    .to_string()
                    .to_lowercase()
                    .starts_with(&typed_prefix_lower)
            }
        })
        .map(|c| {
            let label = c.label.to_string();
            let raw_apply_text = c
                .apply
                .as_ref()
                .map(|s| s.to_string())
                .unwrap_or_else(|| label.clone());

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

    // 5. Coaching: Physical dimensions & units heuristics
    if !typed_prefix.is_empty() && is_physical_dimension_context_ast(source, cursor_index) {
        if typed_prefix.parse::<f64>().is_ok() {
            let units = ["pt", "em", "cm", "mm"];
            for unit in units {
                let suggested_text = format!("{}{}", typed_prefix, unit);
                results.insert(
                    0,
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

    CompletionResponse {
        replace_range,
        items: results,
    }
}

/// Backward-compatible wrapper returning only the completion items.
pub fn get_enhanced_completions(
    world: &dyn IdeWorld,
    document: Option<&PagedDocument>,
    source: &Source,
    cursor_index: usize,
) -> Vec<TypastryCompletion> {
    get_enhanced_completions_with_range(world, document, source, cursor_index).items
}

/// Checks if the cursor is in the value position of a dimension-expecting named argument
/// (e.g. `#rect(width: 10|)`), avoiding false positives in markup or math mode.
pub fn is_physical_dimension_context_ast(source: &Source, cursor_index: usize) -> bool {
    let content = source.text();
    if cursor_index == 0 || cursor_index > content.len() || !content.is_char_boundary(cursor_index)
    {
        return false;
    }

    let root = LinkedNode::new(source.root());
    let Some(leaf) = root
        .leaf_at(cursor_index, Side::Before)
        .or_else(|| root.leaf_at(cursor_index, Side::After))
    else {
        return false;
    };

    let mut current = Some(leaf);
    while let Some(node) = current {
        match node.kind() {
            // Stop if we hit a nested content/math/markup block before reaching a Named arg.
            SyntaxKind::ContentBlock | SyntaxKind::Markup | SyntaxKind::Math => return false,
            SyntaxKind::Named => {
                // Confirm the cursor is after the `:` separator (value position).
                let colon_end = node
                    .children()
                    .find(|c| c.kind() == SyntaxKind::Colon)
                    .map(|c| c.range().end);

                let Some(colon_end) = colon_end else {
                    return false;
                };
                if cursor_index < colon_end {
                    return false;
                }

                let Some(named_ast) = node.cast::<ast::Named>() else {
                    return false;
                };

                return matches!(
                    named_ast.name().as_str(),
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
                );
            }
            _ => {}
        }
        current = node.parent().cloned();
    }

    false
}

/// Backward-compatible wrapper around `is_physical_dimension_context_ast`.
pub fn is_physical_dimension_context(content: &str, offset: usize) -> bool {
    let source = Source::detached(content);
    is_physical_dimension_context_ast(&source, offset)
}

/// Computes the exact byte replacement range and whether the trigger is a `#` command
/// using the `typst-syntax` CST.
pub fn get_trigger_range_ast(source: &Source, cursor_index: usize) -> (Range<usize>, bool) {
    let content = source.text();
    let clamped = cursor_index.min(content.len());
    if clamped == 0 || !content.is_char_boundary(clamped) {
        return (clamped..clamped, false);
    }

    let root = LinkedNode::new(source.root());
    let Some(leaf) = root.leaf_at(clamped, Side::Before) else {
        return (clamped..clamped, false);
    };

    match leaf.kind() {
        SyntaxKind::Hash => (clamped..clamped, true),
        SyntaxKind::Ident | SyntaxKind::Int | SyntaxKind::Float => {
            let start = leaf.range().start;
            let is_hash = leaf
                .prev_sibling()
                .map(|p| p.kind() == SyntaxKind::Hash)
                .unwrap_or_else(|| start > 0 && content[..start].ends_with('#'));
            (start..clamped, is_hash)
        }
        SyntaxKind::Dot => (clamped..clamped, false),
        _ => (clamped..clamped, false),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_package_import_context_ast() {
        // Single-line unclosed import
        let s1 = Source::detached("#import \"@preview/cetz:");
        let res1 = detect_package_import_context_ast(&s1, s1.text().len());
        assert_eq!(res1, Some(("@preview/cetz:".to_string(), 9..23)));

        // Preceding string/comment on same line
        let s2 = Source::detached("\"hello\" #import \"@preview/");
        let res2 = detect_package_import_context_ast(&s2, s2.text().len());
        assert_eq!(res2, Some(("@preview/".to_string(), 17..26)));

        // Multi-line import statement with imported items
        let s3 = Source::detached("#import \"@preview/cetz:0.2.0\": (\n  canvas,\n  draw,\n)");
        // Cursor at index 22 is inside the string right after `@preview/cetz`
        let res3 = detect_package_import_context_ast(&s3, 22);
        assert_eq!(res3, Some(("@preview/cetz".to_string(), 9..22)));

        // Cursor at index 29 (after the closing quote) should not trigger
        assert_eq!(detect_package_import_context_ast(&s3, 29), None);
    }

    #[test]
    fn test_dimension_context_ast() {
        // Valid dimension parameter
        let s1 = Source::detached("#rect(width: 10");
        assert!(is_physical_dimension_context_ast(&s1, s1.text().len()));

        // False-positive avoidance in normal markup
        let s2 = Source::detached("Text with size: 10");
        assert!(!is_physical_dimension_context_ast(&s2, s2.text().len()));

        // False-positive avoidance in math mode
        let s3 = Source::detached("$ 1 + 2 $");
        assert!(!is_physical_dimension_context_ast(&s3, 5));

        // False-positive avoidance inside content block of a named arg
        let s4 = Source::detached("#rect(width: [size: 10");
        assert!(!is_physical_dimension_context_ast(&s4, s4.text().len()));
    }

    #[test]
    fn test_trigger_range_ast() {
        let s1 = Source::detached("#re");
        assert_eq!(get_trigger_range_ast(&s1, 3), (1..3, true));

        let s2 = Source::detached("#");
        assert_eq!(get_trigger_range_ast(&s2, 1), (1..1, true));

        let s3 = Source::detached("#page.");
        assert_eq!(get_trigger_range_ast(&s3, 6), (6..6, false));

        // Multi-byte UTF-8 before #ident
        let s4 = Source::detached("— #rect");
        let len = s4.text().len();
        assert_eq!(get_trigger_range_ast(&s4, len), (5..9, true));
    }
}
