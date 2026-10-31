# Tree-Sitter Completion Intelligence Roadmap

This document outlines a phased approach to replacing fragile backward string scans in `typastry::intel` with resilient Tree-Sitter syntactic queries using `tree-sitter-typst`.

---

## Architecture Overview

User Keystroke / Ctrl+Space
│
▼
EditorPanel / CompletionProvider (typforge)
│
▼
typastry::intel::get_enhanced_completions(world, doc, source, cursor, ts_tree)
│
├── 1. Syntactic Context (Tree-Sitter Query)
│      ├── Inside #import "@..." string? ──> Return Package Completions
│      ├── In dimension param (e.g. size: |)? ──> Offer Coaching Units (pt, em, etc.)
│      └── Trigger prefix & node boundaries ──> Compute exact replacement range
│
└── 2. Semantic Completions (typst-ide)
└── Autocomplete results filtered by syntactic node validity

---

## Quick Handoff Prompt (Copy/Paste When Starting Work)

> "We are working on upgrading Typst autocompletion in `crates/typastry/src/intel.rs` using `tree-sitter-typst`. 
> 
> Current state:
> - `tree-sitter-typst` grammar is compiled in `typastry::syntax`.
> - `intel.rs` currently relies on backward string scans (`detect_package_import_context`, `is_physical_dimension_context`, `get_trigger_info`) which break easily on comments, multi-line arguments, or quotes.
> - We want to replace these scans with Tree-Sitter node queries (`ts_node.kind()`, `parent()`) directly inside `intel.rs` while keeping compiler-level suggestions from `typst-ide`.
> 
> Please follow `roadmap/tree_sitter_completions.md` Phase [X]."

---

## Phase 1: Context Detection for Package Imports

### Objective
Replace the heuristic string search in `detect_package_import_context` with a direct Tree-Sitter node check.

### Problem Solved
Currently, `#import "@preview/..."` detection fails if there is a string earlier on the same line or if the import statement spans multiple lines.

### Implementation Details
1. Create a helper in `intel.rs`:
   ```rust
   pub fn detect_package_import_context_ts(
       source_text: &str,
       tree: &tree_sitter::Tree,
       cursor_offset: usize,
   ) -> Option<String>
Locate the node at cursor_offset:
let node = tree.root_node().descendant_for_byte_range(cursor_offset, cursor_offset)?;
Walk parent nodes:
Verify ancestor node kind is import (or module_import).
Extract string contents between opening quote and cursor_offset.
Ensure it begins with @.
Verification Gate (Phase 1)
Add unit tests in intel.rs:
Package import on single line: #import "@preview/cetz:"|
Package import with preceding comments or other strings on the same line.
Multi-line import statement:
#import
  "@preview/cetz:0.2.0"
Pass condition: cargo test -p typastry test_package_import_context_ts passes with 0 failures.
Phase 2: Dimension & Unit Coaching Detection
Objective
Replace is_physical_dimension_context with an AST parameter lookup.
Problem Solved
Typing numbers in equations (e.g., $ 1 + 2 $) or inside regular text strings can falsely trigger dimension suggestions if the word size: appeared earlier in the paragraph.
Implementation Details
Inspect the cursor position within the Tree-Sitter tree:
Confirm cursor is inside an arguments or named_arg node.
Extract the parameter identifier node (e.g. size, margin, radius).
Confirm cursor sits in the value position (after :).
If confirmed, prepend length units (pt, em, cm, mm) to the completion list.
Verification Gate (Phase 2)
Unit tests in intel.rs:
Correct trigger: #rect(width: 10|) -> returns ["10pt", "10em", "10cm", "10mm"].
False-positive avoidance: Text with size: 10| (in normal paragraph markup) must not trigger units.
Pass condition: cargo test -p typastry test_dimension_context_ts passes.
Phase 3: Accurate Trigger Offset & Replacement Range
Objective
Use Tree-Sitter leaf nodes to determine the exact TextEdit replacement range in TypstCompletionProvider (crates/typforge/src/editor/editor_panel/completions.rs).
Problem Solved
Currently, get_trigger_info iterates backward char-by-char to find word starts, which can miscalculate byte boundaries with unicode characters or when editing after # commands.
Implementation Details
Look up the leaf node directly under the cursor:
Identify whether the node is an ident, field_access, or hash.
Read the exact node.start_byte() and node.end_byte().
Compute the exact prefix slice:
let typed_prefix = &source_text[node.start_byte()..cursor_offset];
Return the exact byte range node.start_byte()..cursor_offset for the LSP CompletionTextEdit.
Verification Gate (Phase 3)
Unit tests testing identifiers with dots, dashes, and hashes (e.g. #page.set, foo-bar).
Editor test: Selecting a completion replaces the exact typed word without duplicating # or leaving trailing letters.
Pass condition: cargo test -p typastry and manual verification in the editor.
Phase 4: Wire Through Editor State & Fallback Handling
Objective
Connect the Tree-Sitter tree from EditorState to TypstCompletionProvider and ensure graceful fallbacks.
Implementation Details
In TypstCompletionProvider::completions:
Pass the existing parsed tree (or parse via typastry::syntax::parse_typst(content)).
In get_enhanced_completions:
If tree-sitter parsing succeeds, use Tree-Sitter context detection.
If the parse tree is missing or has catastrophic syntax errors, fall back to the existing character scan methods.
Verification Gate (Phase 4)
Run cargo check --workspace.
Run full application cargo run.
Test typing # in the editor: ensure completion popover opens instantly with no latency regression.
