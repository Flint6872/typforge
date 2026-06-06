# Role & Context
You are an expert Rust software engineer specializing in compiler tools, AST manipulation (specifically Typst's AST), and native GPUI applications. 

We are building **TypForge**, a native, lightweight, pure-Rust Typst word processor using GPUI (the UI engine from Zed). 

### The Objective
We want to transition away from using an embedded LSP architecture (currently implemented via a crate called `typstography` using `tower-lsp` and local TCP/stream connections) to a **native, direct-crate integration**. 

We will create a new, UI-agnostic logic crate called **`typforge-core`** to handle:
1. **AST-based editing & injections** (`edit` module): Porting and upgrading our `injector.rs` logic to avoid styling nesting and smartly merge edits into parent nodes (like `#text`, `#set page`, and other elements).
2. **Editor Intelligence** (`intel` module): Direct integration with the `typst-ide` crate for context-aware autocompletes and hovers.
3. **Document Formatting** (`format` module): Integration with the `typstyle` formatting library.

This modular architecture allows us to run synchronous or simple background task-based compiler checks without the serialization overhead of JSON-RPC and LSP protocol loops, and allows us to easily unit-test our compiler-interaction logic.

---

## Workspace Files to Review
Here are the files defining our current state (please read these before beginning):
1. **`Cargo.toml` (Workspace)**: To see our crate dependencies.
2. **`crates/typforge/Cargo.toml`**: To see the dependencies of our main app.
3. **`crates/typforge/src/ribbon/injector.rs`**: Our current, nested AST-based editor injection script.
4. **`crates/typforge/src/typst_world.rs`**: Our `typst::World` implementation (`GpuiWorld`).
5. **`crates/typforge/src/main.rs`**: To see how the runtime, the `world`, and the previous LSP were initialized.

---

## Transition Plan: Phased Approach

We will execute this in four systematic phases. **Do not write all code at once.** Write the code for Phase 1, ask me to run tests/verification, and we will move to the next phase only when we are confident.

### Phase 1: Initialize the `typforge-core` Crate
1. Add `typforge-core` to the workspace `Cargo.toml` members.
2. Create `crates/typforge-core` directory containing:
   - `Cargo.toml`
   - `src/lib.rs`
   - `src/edit.rs`
   - `src/intel.rs`
   - `src/format.rs`
3. Configure `crates/typforge-core/Cargo.toml` with:
   - `typst = "0.14.2"` (or matching workspace version)
   - `typst-ide = "0.14.2"` (matches typst version)
   - `typstyle = "0.12"` (or compatible typst-0.14 parser formatter)
   - `typst-syntax` (for AST manipulations)
   - `serde` and standard workspace settings.
4. Set up the basic module exports in `src/lib.rs`.

**Phase 1 Checkpoint:** 
Provide the modified Cargo files and verify that `cargo check` compiles the blank skeleton.

---

### Phase 2: Upgraded AST Injector & Unit Testing (`src/edit.rs`)
1. Port the existing `apply_ribbon_action` logic from `crates/typforge/src/ribbon/injector.rs` into `typforge-core/src/edit.rs`.
2. **Fix the Nesting Bug**: Refactor `apply_text_param_ast` and its helpers. If a user tries to apply a text parameter (like `fill`, `size`, or `font`):
   - Find if the selection is already enclosed by, matches, or contains a `#text` (or other formatting) node.
   - If a target node exists and we are at a cursor selection or selecting the whole node, **merge the new parameter** into its existing arguments list (e.g., `#text(font: "Inter")[Hello]` becomes `#text(font: "Inter", fill: red)[Hello]`) instead of wrapping it (e.g. `#text(fill: red)[#text(font: "Inter")[Hello]]`).
   - If no target exists or it's a small sub-selection of a larger text block, proceed with wrapping it in a brand-new `#text` call.
3. Write comprehensive, UI-free unit tests at the bottom of `src/edit.rs` verifying:
   - Toggling strong/emph formatting.
   - Nesting prevention when changing different attributes on the same selection.
   - Safe argument replacement inside `#set page(...)` elements.

**Phase 2 Checkpoint:**
Once this is implemented, ask me to run `cargo test -p typforge-core` to ensure the AST modification logic passes all safety checks.

---

### Phase 3: Typst-IDE Integration (`src/intel.rs` & `src/format.rs`)
1. In `src/intel.rs`, write a clean interface to query completions and hover details.
   - Implement `get_completions(world, document, source, cursor_index)` using `typst_ide::autocomplete`.
   - Implement `get_hover_info(world, document, source, cursor_index)` using `typst_ide::hover`.
2. In `src/format.rs`, implement document formatting using `typstyle`:
   - Implement `format_document(content: &str) -> Result<String, String>`.

**Phase 3 Checkpoint:**
Verify that the intelligence module compiles successfully and matches the Typst API version characteristics.

---

### Phase 4: Clean up & Integration in `typforge` App
1. Modify `crates/typforge/Cargo.toml`:
   - Remove `tower-lsp`, `lsp-types`, and the old `typstography` crate path.
   - Add `typforge-core = { path = "../typforge-core" }`.
2. Clean up `crates/typforge/src/main.rs`:
   - Delete the Tokio runtime setup (or simplify it), the duplex stream initialization, and the `LspClient` start-up block.
   - Retain and cleanly instantiate the `shared_world` context.
3. Show me how we can map any necessary editor/ribbon actions in our GPUI panels to call the synchronous/async functions in our new `typforge-core` crate directly.

---

Let's begin with **Phase 1**. Give me the configuration files and file paths needed to set up the skeleton of `typforge-core`.
