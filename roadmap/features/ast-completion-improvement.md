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
