# Role and Objective
You are an expert Rust systems engineer and compiler toolchain developer specializing in Concrete Syntax Trees (CST), compiler-backed IDE tooling, and zero-socket embedded editor architecture.

We are building and refining **Typastry**, a pure-Rust, offline-first AST and IDE completions/tooling crate. It is designed as a lightweight, zero-latency alternative to running an external LSP process (`tinymist` / `typst-lsp`) to power multiple distinct desktop frontends:
1. **GPUI Frontend:** For our own high-performance, custom GPU-rendered text editor (`TypForge`).
2. **GTK Frontend:** For cross-platform desktop integration with `Typesetter` (using `gtk4-rs`).

Because of this dual-frontend requirement, **all data structures and inspection APIs must be 100% UI-agnostic, synchronous, and `serde`-serializable**, allowing either GPUI or GTK to consume completions, signature tooltips, and AST node queries natively.

---

# Current Implementation Baseline
We are leveraging `typst` compiler internals directly in-process (`typst-syntax`, `typst-ide`, `typst-kit`, `typst-layout`). Our current crate handles:
1. **Package Discovery:** Scanning system data and cache directories for locally cached/installed Typst packages (`typst_kit::packages::FsPackages`).
2. **Raw Completion Wrapping & Filtering:** Querying `typst_ide::autocomplete` and filtering/mapping suggestions to a portable, serializable format (`TypastryCompletion`).
3. **Coaching Heuristics:** Automatically injecting physical dimension/unit suggestions (`pt`, `em`, `cm`, `mm`) when typing numeric values in parameter contexts like `margin:`, `width:`, `inset:`, etc.
4. **Trigger Offset Detection:** Backwards-scanning logic to find word boundaries and hash command triggers (`#`).

---

# Architectural Roadmap & Tasks
Please help me implement the following modules and functions in idiomatic, high-performance, safe Rust:

## Task 1: Context-Aware Argument Detection (`detect_inside_argument_context`)
- Using `typst-syntax` and `LinkedNode` traversal, write a helper function that checks if the current cursor offset is actively inside a function call's argument list (`Args` node).
- Differentiate between top-level markup/code and nested argument lists (e.g., `#rect(fi|)` vs `#rect() |`), so our completion mapper knows *not* to blindly append `()` when autocompleting inside an existing function call.

## Task 2: Function Signature & Active Parameter Help (`get_signature_help`)
- Model Tinymist's signature help directly in-process without LSP overhead, returning UI-agnostic structs suitable for binding to both GPUI overlays and GTK popovers.
- Given `&dyn IdeWorld`, `Option<&PagedDocument>`, `&Source`, and `cursor_offset`:
  1. Walk up from the cursor leaf node via `LinkedNode` to find the nearest enclosing `FuncCall` and its `Args` node.
  2. Extract the callee function name/path and count top-level commas (ignoring commas inside nested brackets/parens) before the cursor to compute `active_parameter_index`.
  3. Resolve the function's signature, parameter names, types/defaults, and documentation using `typst-ide` / evaluated scope metadata.
  4. Return a clean, serializable `TypastrySignatureHelp` struct containing the function name, overall doc, a `Vec<ParameterInfo>`, and `active_parameter_index`.

## Task 3: AST-Anchored Parameter Insertion / Updating (`inject_or_update_parameter`)
- Write a robust utility function for our UI Ribbons (usable by both GPUI toolbars and GTK action buttons): given a source string, a target byte offset, a parameter name (e.g., `fill`), and a value (e.g., `red`), it should:
  1. Parse the document into a CST using `typst-syntax::parse`.
  2. Locate the nearest enclosing `FuncCall` and its `Args` child node.
  3. Check if the parameter is already defined. If so, replace its value expression span cleanly; if not, splice the new `param: value` into the argument list (handling empty `()`, trailing commas, and spacing properly).
  4. Return the updated source string without fragile line/column math.

## Task 4: Lightweight Cursor & Canvas Inspector (`TypastryInspector`)
- Design a fast, synchronous `TypastryInspector` struct that inspects a source string and byte offset to return a structured `CursorContext` enum:
  - `InMarkup`
  - `InCode`
  - `InFunctionCall { name: String, supplied_named_args: Vec<String>, active_param_index: usize }`
  - `InImportPath(String)`
- Include clean wrappers around `typst_ide::jump_from_click` and `typst_ide::tooltip` to support bidirectional canvas-to-AST selection and live value previews.

---

# Constraints & Design Philosophy
- **Zero Framework Coupling:** Do not import `gpui`, `gtk`, or any UI-specific crates into this library crate. Keep it pure business logic and AST processing so it compiles instantly and links cleanly into both projects.
- **Zero Heavy Runtimes:** Keep everything synchronous and designed for embedded desktop execution.
- **Error Tolerance:** Typst documents are frequently in incomplete/malformed states while typing. All AST inspections must be error-tolerant, leaning on `typst-syntax` resilience and avoiding panics.






Phased Implementation Plan for typastry
Phase 1: AST-Backed Context & Replacement Ranges (intel.rs)
Replace the fragile backward character scans in intel.rs with LinkedNode traversal on source.root():
Package Import Detection (detect_package_import_context):
Walk up from root.leaf_at(cursor_index, Side::Before).
Confirm an ancestor is SyntaxKind::ModuleImport and the cursor sits inside a SyntaxKind::Str literal starting with @.
Handles multi-line #import, preceding comments, and other strings on the same line without false positives.
Dimension Coaching Detection (is_physical_dimension_context):
Walk up from the cursor leaf to check for SyntaxKind::Named inside SyntaxKind::Args (and ensure we are not inside SyntaxKind::Math or plain SyntaxKind::Markup).
Verify the cursor is in the value expression (after Colon) and match named.name().as_str() against dimension-expecting parameters (size, margin, width, height, radius, stroke, inset, outset, gap, gutter, spacing).
Exact Replacement Ranges (CompletionResponse):
Add a TypastryCompletionResult (or CompletionResponse) struct:
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct CompletionResponse {
    pub replace_range: std::ops::Range<usize>,
    pub items: Vec<TypastryCompletion>,
}
Compute replace_range from the leaf node (Ident, FieldAccess, Str, or Hash) at cursor_index (node.range().start..cursor_index), avoiding UTF-8 / # duplication bugs.
Keep get_enhanced_completions working (or provide get_enhanced_completions_v2 / update TypForge in tandem).
Phase 2: Argument-Context Awareness & Pre-Compile Speed (intel.rs)
detect_inside_argument_context(source: &Source, cursor: usize) -> ArgumentContext:
Use LinkedNode to determine if the cursor is inside SyntaxKind::Args of a FuncCall / SetRule / ShowRule (and not inside a nested ContentBlock [...] unless it’s a new #call).
Use this in the completion mapper:
Inside Args (e.g., #rect(fi|)): do not append () to parameter/variable completions.
Inside ContentBlock / Markup / top-level Code: append () for CompletionKind::Func as before.
Pre-Compile Resilience (document: Option<&PagedDocument> is None):
typst_ide::autocomplete already runs without a compiled PagedDocument using IdeWorld + Source.
Ensure package completions, unit coaching, and prefix filtering short-circuit or merge cleanly even when typst_ide::autocomplete returns None or partial results on malformed syntax.
Phase 3: Function Signature Help (intel.rs or signature.rs)
Add synchronous, serde-serializable signature help:
Data Types:
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct TypastryParameterInfo {
    pub name: String,
    pub doc: Option<String>,
    pub default: Option<String>,
    pub is_positional: bool,
    pub is_variadic: bool,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct TypastrySignatureHelp {
    pub func_name: String,
    pub doc: Option<String>,
    pub parameters: Vec<TypastryParameterInfo>,
    pub active_parameter: usize,
}
get_signature_help(world: &dyn IdeWorld, document: Option<&PagedDocument>, source: &Source, cursor: usize) -> Option<TypastrySignatureHelp>:
Walk up via LinkedNode to find the nearest enclosing FuncCall (or SetRule) whose Args span contains cursor.
Compute active_parameter by counting top-level SyntaxKind::Comma children in Args before cursor (or matching Named arg name if the cursor is on a named argument).
Resolve the callee via typst_ide::analyze_expr (using world / source—works pre-compile!) or fall back to world.library().global.scope().get(&name) for stdlib functions when expression analysis fails mid-edit.
Extract Func::params() (ParamInfo: name, docs, default, positional, variadic) into TypastrySignatureHelp.
Phase 4: Unified AST Parameter Mutation (edit.rs & wysiwyg.rs)
Unify apply_named_argument_edit (wysiwyg.rs) and the ad-hoc string splitting in edit.rs (get_arg_value, merge_args) into a single AST-backed helper:
inject_or_update_parameter(content: &str, offset: usize, key: &str, value: &str) -> Option<TextEdit>:
Return TextEdit (range, new_text, new_selection) so frontends apply localized edits instead of cloning/replacing the whole buffer.
Walk LinkedNode to find the nearest enclosing FuncCall / SetRule and its Args child.
If key exists as a SyntaxKind::Named child: replace only its value expression span (preserving surrounding formatting/comments).
If key does not exist: insert , key: value (or key: value if empty ()) before RightParen (handling trailing commas cleanly).
Keep wysiwyg::apply_named_argument_edit as a thin wrapper if needed during transition, or migrate callers to TextEdit.
Phase 5: TypastryInspector (inspector.rs)
Create a unified, UI-agnostic inspection API for both GPUI and GTK:
CursorContext enum (Serialize, Deserialize):
InMarkup
InCode
InMath
InFunctionCall { name: String, supplied_named_args: Vec<String>, active_param_index: usize }
InImportPath(String)
TypastryInspector:
inspect_cursor(source: &Source, offset: usize) -> CursorContext (pure syntax, zero-compile, instant).
tooltip(world, document, source, offset) -> Option<Tooltip> (clean wrapper around typst_ide::tooltip).
jump_from_click(world, document, source, page, point) -> Option<JumpTarget> (serializable wrapper around typst_ide::jump_from_click so neither GPUI nor GTK has to deal with typst_ide::Jump directly).
