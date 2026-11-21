# typASTry 🧶

A high-performance, embedded AST manipulation and intelligence engine for [Typst](https://github.com/typst/typst). 

`typASTry` transforms low-level Typst compiler internals into a high-level, UI-agnostic library designed for custom desktop editor frontends (e.g., GPUI, GTK).

---

## The "Tinymist" Alternative for Embedded Editors
Unlike LSPs which require external process management and JSON-RPC overhead, `typASTry` operates **in-process and synchronously**. It provides "LSP-grade" intelligence—completions, signature help, and AST mutation—directly to your editor's main thread with microsecond latency.

## Key Capabilities

* **Context-Aware Intelligence:** AST-backed completion, signature help, and parameter injection that understands when you're inside a function call vs. markup.
* **Zero-Socket Architecture:** Built for high-performance editors that require sub-millisecond AST feedback.
* **Smart AST Mutation:** Methods that modify function arguments and set rules without destroying document formatting or creating nested style artifacts.
* **Pre-Compile Resilience:** Intelligence features function even when the document is in an incomplete state and hasn't yet produced a valid `PagedDocument`.

---

## Quick Start: Integrating `typASTry`

`typASTry` requires an implementation of `typst::World` (usually provided by your editor's document state).

### 1. Context-Aware Completions
Use `get_enhanced_completions` to receive a `TypastryCompletionResponse`. This includes an exact `replace_range` so your editor can perform correct text edits without index math errors.

```rust
use typastry::intel::get_enhanced_completions;

let response = get_enhanced_completions(&world, None, &source, cursor_offset);

for item in response.items {
    // Label: "fill", Apply: "fill: ", etc.
    // Use response.replace_range to apply text_edit
}
```

### 2. Live Signature Help
Display function parameters and active arguments as the user types.

```rust
use typastry::intel::get_signature_help;

if let Some(help) = get_signature_help(&world, None, &source, cursor_offset) {
    println!("Function: {}", help.func_name);
    println!("Active Param: {}", help.parameters[help.active_parameter].name);
}
```

### 3. AST-Anchored Parameter Injection
Instead of regex-based editing, use the CST to safely inject or update parameters.

```rust
use typastry::edit::inject_or_update_parameter;

// Safely adds/updates 'fill' to a #rect() call at the cursor
if let Some(edit) = inject_or_update_parameter(source_text, cursor, "fill", "red") {
    // edit.new_text: "fill: red"
    // edit.range: The span to replace
}
```

### 4. Visual Inspector
Get the current cursor context (Markup, Code, Math, or Function Call) for status bar or ribbon updates.
```rust
use typastry::inspector::TypastryInspector;

let context = TypastryInspector::inspect_cursor(&source, cursor);
match context {
    CursorContext::InFunctionCall { name, active_param_index, .. } => {
        // Toggle ribbon buttons for function-specific properties
    }
    _ => {}
}
```

5. Syntax Utilities (syntax)
typASTry bundles optimized Tree-sitter grammars and configuration for both Typst and BibTeX. This allows direct access to tree_sitter::Language instances, syntax highlight/injection queries, and auto-closing bracket/quote pairs. It provides a comprehensive, embedded solution for fundamental editor syntax features.

```rust
use typastry::syntax::{typst_language, create_typst_parser};
use typastry::syntax::{BIBTEX_LANGUAGE, BIBTEX_HIGHLIGHTS_QUERY}; // Corrected to use BIBTEX_LANGUAGE
use typastry::syntax::{TYPST_AUTO_CLOSING_PAIRS, BIBTEX_AUTO_CLOSING_PAIRS};

// Get the Typst Tree-sitter language and parser
let typst_lang = typst_language();
let mut typst_parser = create_typst_parser();
let typst_tree = typst_parser.parse("#let x = 1", None);

// Access Typst highlight queries and auto-closing pairs
let typst_highlights = TYPST_HIGHLIGHTS_QUERY;
for (open, close) in TYPST_AUTO_CLOSING_PAIRS {
    println!("Typst Auto-close: {} -> {}", open, close);
}

// Get the BibTeX Tree-sitter language and highlight query
let bibtex_lang = BIBTEX_LANGUAGE; // Assuming BIBTEX_LANGUAGE is exposed
let bibtex_highlights = BIBTEX_HIGHLIGHTS_QUERY;
for (open, close) in BIBTEX_AUTO_CLOSING_PAIRS {
    println!("BibTeX Auto-close: {} -> {}", open, close);
}
```
### Architectural Philosophy
- UI-Agnostic: No dependencies on gpui or gtk.
- Compile-time Modularity: Toggle intel or format features to keep the binary footprint minimal for restricted environments.
- Error Tolerance: Built to process incomplete or malformed code during active keystrokes, leveraging typst-syntax resilience.

### Feature Flags

| Feature | Default | Description |
| -------- | -------- | -------- |
| intel    | Yes     | Completion coaching, signature help, and AST inspection.     |
| format    | Yes     | Native typstyle document formatting integration.     |

### Contributing
We welcome contributions for more AST presets! Please open a PR to add:
- Smart table/grid manipulators.
- Advanced markdown conversions.
- Creative autocomplete coaching heuristics.
