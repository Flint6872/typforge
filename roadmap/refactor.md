I am refactoring TypForge (a native Typst desktop editor built with GPUI). 
The workspace is organized into crates:
- `typforge`: App shell, window management, panels, dock layout, and GPUI UI views.
- `typst-gpui`: Canvas renderer and pointer/hit testing for Typst `PagedDocument`.
- `typastry`: AST inspection, syntax mutations, formatting, and spellcheck.
- `typsdocx`: MS Word docx export.
- `typforge-core` (NEW): Headless core library holding project/buffer state, the Typst `World` implementation, compilation pipeline, edit revisions, and diagnostics. No GPUI dependencies.

### Architectural Goals:
1. High-responsiveness, zero-lag typing in both the code editor and preview pane with clean bi-directional sync.
2. Eliminate UI-thread lock contention (`Arc<Mutex<World>>` is currently locked during full compiles while the main UI thread tries to query it).
3. Eliminate duplicate state and asymmetric debounce (editor text, preview text, and world source text are currently desynchronized; preview-to-editor uses a 150ms debounce that causes race conditions).
4. Strip compiler, export, and spellcheck orchestration out of `typst-gpui::PreviewPanel` so that `typst-gpui` is strictly a reactive document renderer.

### Next Step:
We are starting Phase 1: populating `typforge-core` with the Typst `World` and compiling standalone without GPUI dependencies.


1. Refactor vs. Fresh Start
Verdict: Refactoring into typforge-core is practical and distinctly cleaner than a fresh start.
Why: typst_world.rs is already almost completely free of GPUI dependencies (it only uses standard Rust, typst, typst_kit, and traits). The GPUI UI tree (workspace/mod.rs, dock system, ribbon) and the low-level Typst frame rendering in typst-gpui are functional.
The real issue: Business logic, buffer state, and compilation orchestration are currently smeared across PreviewPanel and workspace/mod.rs. You don’t need a rewrite; you need an extraction of the engine layer.
2. Primary Lag & Sync Bottlenecks
Lock Contention on UI Thread (Arc<Mutex<World>>):
In PreviewPanel::compile, world.lock() is held during the entire typst::compile call on a background thread.
Concurrently, the UI thread locks shared_world in workspace/mod.rs (e.g., in DiagnosticsChanged to resolve source lines for squiggles) and in editor components. If compilation takes 80–200ms, the UI thread locks and drops frames.
Three Competing Sources of Truth & Asymmetric Debounce:
Text is stored separately in: EditorState (GPUI input state), PreviewPanel.source, and GpuiWorld.source_text.
Canvas typing compiles immediately, but syncs to the editor via a 150ms debounce task (editor_sync_task). If the user edits canvas text and then immediately types in the editor or clicks a Ribbon formatting tool before 150ms elapses, the states overwrite each other or get clobbered.
Full Synchronous String Cloning on Keystrokes:
Every keystroke in either pane passes whole String allocations across channels/events (FileContentUpdated, PreviewPanelEvent::SourceChanged).
Scope Creep in PreviewPanel:
PreviewPanel acts as an AST inspector, spellchecker (Harper), font registrar, and compilation pipeline. Frame hitches occur when font registration and diagnostic emissions fire simultaneously back into the main thread.
3. Boundary & Coupling Risks
typst-gpui Inverted Responsibilities:
PreviewPanel currently defines TypstGpuiWorld and controls typst::compile, spellchecking, PDF/DOCX export, and World mutations.
Target boundary: typst-gpui should purely be a reactive view rendering an immutable Arc<PagedDocument>, emitting canvas interaction events (caret hit testing, clicks, raw keystrokes). It should not own World or know how compilation works.
UI-Coupled Diagnostics & Language Tools:
Spellchecking (Harper), Typst diagnostics filtering, and AST inspection (typastry) are currently triggered from GPUI view callbacks.
These belong in typforge-core behind a unified, non-GPUI CompilationHost or Workspace engine that produces clean diagnostic events.
Arc<Mutex<W>> Threading Model:
Typst’s World requires immutable access (&self) during compilation, but mutating sources requires &mut self. Holding a long-lived parking_lot::Mutex across UI and background threads makes lock inversion and UI stalls inevitable.
