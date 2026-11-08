Overall Goal
To evolve the diagnostic system from Typst-compiler-centric to a pluggable architecture that supports multiple diagnostic sources (Typst, BibTeX, potentially YAML, image file errors, etc.) and presents them consistently in the UI.
The Current “Image Error” Problem
The error EditorPanel: Failed to open file "...\countdown.gif": stream did not contain valid UTF-8 arises because your editor tries to read all files into memory as UTF-8 text using fs::read_to_string when they are opened. For binary files like images, this fails, leading to a console error but no visual feedback in the UI. We’ll address this by:
Recognizing file types (e.g., via extension).
If it’s not a text file, or if the UTF-8 read fails, creating a custom EditorDiagnostic for display.
Files to Review Before Starting
Please review these files carefully, as they are central to the diagnostic flow and will be modified in the upcoming phases:
crates/typforge/src/editor/mod.rs: Defines OpenedFile, get_language_from_path.
crates/typforge/src/editor/code_editor.rs: Handles displaying squiggly lines via set_diagnostics, has clear_diagnostics.
crates/typforge/src/editor/diagnostics_panel.rs: Displays the list of diagnostics.
crates/typforge/src/editor/editor_panel/render.rs: Renders the tab UI, including the red error dot.
crates/typforge/src/workspace/mod.rs: Manages TypstNoteView, currently subscribes to PreviewPanelEvent::DiagnosticsChanged and distributes diagnostics.
crates/typforge/crates/typst-gpui/src/lib.rs: Defines PreviewPanel and emits PreviewPanelEvent::DiagnosticsChanged.
crates/typforge/src/typst_world.rs: Defines GpuiWorld, which resolves FileIds.
Phased Approach
Here’s the plan to move forward:
Phase 1: Standardize EditorDiagnostic Type
Goal: Introduce a common EditorDiagnostic struct that all UI components will understand and use, decoupling them from typst::diag::SourceDiagnostic.
Steps:
Define EditorDiagnostic:
Create a new module, e.g., crates/typforge/src/diagnostics/mod.rs, to house this common type.
The EditorDiagnostic struct will be a generalized representation of a diagnostic message, including:
severity (Error, Warning, Info - your own enum).
message (String).
file_path (PathBuf).
range (std::ops::Range<usize>) - the byte range in the file.
source_name (String, e.g., “Typst Compiler”, “BibTeX Linter”, “File Handler”).
hints (Vec<String>).
trace (Vec<String>).
Include conversion logic to map typst::diag::SourceDiagnostic to this new EditorDiagnostic type.
Update OpenedFile:
In crates/typforge/src/editor/mod.rs, change OpenedFile.diagnostics: Vec<typst::diag::SourceDiagnostic> to Vec<EditorDiagnostic>.
Update CodeEditor:
In crates/typforge/src/editor/code_editor.rs, modify set_diagnostics to accept Vec<EditorDiagnostic> instead of Vec<SourceDiagnostic>.
Inside set_diagnostics, convert EditorDiagnostics to gpui_kit::component::input::Diagnostic for rendering squiggly lines. The range mapping logic will need to be adapted to use the EditorDiagnostic.range.
Update DiagnosticsPanel:
In crates/typforge/src/editor/diagnostics_panel.rs, modify set_diagnostics to accept Vec<EditorDiagnostic>.
Adjust resolve_diagnostic (or create a new helper) to convert EditorDiagnostics to your ResolvedDiagnostic type for display in the panel. The filtering logic for .typ files can remain here or be moved to the central manager (we’ll make a final decision on this in a later phase for flexibility).
Update PreviewPanel:
In crates/typforge/crates/typst-gpui/src/lib.rs, the compile method emits PreviewPanelEvent::DiagnosticsChanged. Modify this method to convert the typst::diag::SourceDiagnostics into Vec<EditorDiagnostic> before emitting the PreviewPanelEvent::DiagnosticsChanged event.
The PreviewPanelEvent itself will need to be updated to carry Vec<EditorDiagnostic>.
Adjust TypstNoteView’s Subscription:
In crates/typforge/src/workspace/mod.rs, update the cx.subscribe handler for PreviewPanelEvent::DiagnosticsChanged to receive Vec<EditorDiagnostic>.
Temporarily remove the explicit .typ filtering logic from this handler; all incoming diagnostics will already be EditorDiagnostics. The has_diagnostics flag will now simply check if the list of EditorDiagnostics is empty.
The distribution logic will need to correctly match EditorDiagnostic.file_path to OpenFile.path.
Confirmation: After Phase 1, the editor should still display Typst diagnostics as squiggly lines, in the diagnostics panel, and on Typst tab indicators, but now using the new EditorDiagnostic type internally. Opening .bib files should still not show a red dot.
Phase 2: Centralized Diagnostic Management & BibTeX Integration
Goal: Introduce a central diagnostic manager to collect diagnostics from all sources and integrate a basic BibTeX diagnostic provider.
Steps:
Create DiagnosticManager:
In crates/typforge/src/diagnostics/mod.rs, define a DiagnosticManager struct. This manager will be responsible for holding the current state of diagnostics for all open files. A HashMap<PathBuf, Vec<EditorDiagnostic>> is a good starting point, mapping file paths to their diagnostics.
The manager will have methods to set_diagnostics_for_file(path: PathBuf, diagnostics: Vec<EditorDiagnostic>) and get_diagnostics_for_file(path: &PathBuf) -> Vec<EditorDiagnostic>.
It should also have a method to clear_all_diagnostics().
Integrate DiagnosticManager into TypstNoteView:
In crates/typforge/src/workspace/mod.rs, TypstNoteView will own an Entity<DiagnosticManager>.
The PreviewPanelEvent::DiagnosticsChanged handler in TypstNoteView will now use the DiagnosticManager to update the Typst diagnostics for affected files. Instead of iterating panel.open_files directly, it will update the manager, and then the manager will be queried to populate the OpenFile.diagnostics and CodeEditor.set_diagnostics.
Create BibTeX Diagnostic Provider:
Create a new module, e.g., crates/typforge/src/diagnostics/bibtex_provider.rs.
Implement a service or function (e.g., run_bibtex_diagnostics(path: &PathBuf, content: &str) -> Vec<EditorDiagnostic>) that uses your tree-sitter-bibtex parser to find syntax errors in .bib files.
This function will convert any Tree-sitter errors (or custom linting errors) into EditorDiagnostic instances.
Trigger BibTeX Diagnostics:
Identify where files are opened or saved in your editor (likely within EditorPanel). When a .bib file is opened or its content changes, call the run_bibtex_diagnostics function.
The results (a Vec<EditorDiagnostic>) will then be passed to the DiagnosticManager for that specific .bib file path.
Update UI Consumers to Query Manager:
TypstNoteView’s logic for populating OpenFile.diagnostics and calling CodeEditor.set_diagnostics will now query the DiagnosticManager for the diagnostics associated with the relevant file path.
The DiagnosticsPanel will query the DiagnosticManager for all diagnostics currently present.
Confirmation: After Phase 2, opening a .bib file with syntax errors should now:
Show a red dot on its tab.
Display squiggly lines in the editor.
List the BibTeX diagnostics in the Diagnostics Panel (potentially alongside Typst diagnostics if a .typ file is also open with errors).
Phase 3: Handle Non-Text File Opening Errors (Image)
Goal: Catch and visually represent errors when trying to open non-text files as text, or when a file isn’t valid UTF-8.
Steps:
Identify File Opening Logic:
Locate the OpenedFile::new(path: PathBuf, ...) function in crates/typforge/src/editor/mod.rs (or EditorPanel::open_file if that’s where the read happens). This is where fs::read_to_string is called.
Implement Robust File Reading:
Before attempting fs::read_to_string(&path):
Check the file extension: If it’s a known binary/image format (e.g., .png, .jpg, .gif, .pdf), generate an EditorDiagnostic (e.g., “Cannot open binary file for editing”) and immediately add it to the DiagnosticManager for that path.
If it’s not a known binary/image, try fs::read_to_string.
If fs::read_to_string returns an error (specifically std::io::ErrorKind::InvalidData for UTF-8 issues), generate an EditorDiagnostic (e.g., “File is not valid UTF-8 text”) and add it to the DiagnosticManager.
For files that cannot be opened as text, ensure they are still registered as OpenedFiles, perhaps with an empty EditorState or a placeholder, so the tab exists and can display the diagnostic.
Update DiagnosticManager:
Ensure the DiagnosticManager can accept and store these new “File Handler” diagnostics.
Display New Diagnostics:
Since all UI components already consume EditorDiagnostics (from Phase 1), these new file-opening diagnostics will automatically appear in the diagnostics panel, as squiggly lines (if a valid range can be given, otherwise possibly just a file-level diagnostic), and on the tab indicator.
Confirmation: After Phase 3, opening a binary file (like countdown.gif) should:
Show a red dot on its tab.
Display a diagnostic message in the Diagnostics Panel indicating it’s a binary file or invalid UTF-8.
No longer print stream did not contain valid UTF-8 to the console.
This detailed plan will guide us through a robust and extensible diagnostic system. We’ll start with Phase 1 when you’re ready, focusing on EditorDiagnostic.
