You are an expert Rust (2024 edition) and GPUI engineer helping me implement "Portal Edit Mode" for `#include "file.typ"` sections in a high-performance native Typst WYSIWYG editor.

### Workspace Architecture (3 Crates)
- **`typastry`**: Manages AST structures, source mapping, and Typst `Span` metadata (`FileId` + byte offsets).
- **`typst-gpui`**: The preview pane crate. Handles rendering, layout frames, visual elements, and spatial hit-testing against rendered nodes.
- **`Typforge`**: The binary application crate. Manages the dual-pane UI (Editor Pane + Preview Pane), global state, and buffer synchronization using `ropey`.

### Feature Workflow: Portal Edit Mode for `#include`
1. **Trigger:** Clicking an `#include` section boundary or shortcut in the Preview Pane (`typst-gpui`).
2. **Visual Isolation:** The preview dims background elements (~40-50% opacity), frames the include section with an accent border, and displays a file badge (e.g., `📁 file.typ`).
3. **Editor Sync:** The Editor Pane (`Typforge`) focuses/swaps to the target sub-file buffer.
4. **Exit/Commit:** Pressing `Cmd/Ctrl + Enter` OR clicking anywhere in the dimmed outside area commits the buffer edits, triggers a Typst recompile, and returns to global view.

---

### Multi-Phase Implementation Plan

#### Phase 1: State Machine & Source Mapping (Current Focus)
- Define `EditorMode` state machine (`Global` vs. `PortalEdit { file_id }`) in application state.
- Implement Typst `Span` hit-testing using `typastry` metadata inside `typst-gpui` to map visual mouse clicks to target inclusion files.
- Define clean state transition methods.

#### Phase 2: GPUI Visual Overlay & Boundary Rendering
- Build the GPUI canvas rendering layer for dimmed backgrounds, accent boundary boxes, and path badges within `typst-gpui`.

#### Phase 3: Dual-Pane & Input Trapping
- Synchronize portal focus between `typst-gpui` events and the Editor Pane buffer tabs in `Typforge`.

#### Phase 4: Commit Lifecycle & Incremental Recompilation
- Implement `Cmd/Ctrl+Enter` and outside-click commit flows and trigger incremental recompilation.

---

### Instructions for Response
Please focus EXCLUSIVELY on **Phase 1**. Provide idiomatic Rust 2024 code, respecting my crate boundaries (`typastry`, `typst-gpui`, `Typforge`), for:
1. Core state data structures (`EditorMode`, tracking structs).
2. Typst `Span` hit-test mapping logic inside `typst-gpui` to identify target include files from visual coordinates using `typastry`.
3. Clean state transition methods (`enter_portal`, `commit_portal`).

Do not generate UI rendering or Phase 2-4 code yet. Keep code well-commented and clean.
