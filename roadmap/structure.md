# Typforge Project Structure

This document outlines the structure of the Typforge project, detailing its main components and their responsibilities.

## Workspace Root (`/`)

The top-level directory contains the workspace `Cargo.toml` and documentation.

- `Cargo.toml`: Defines the workspace and its dependencies.
- `roadmap/`: Contains project planning and documentation.
  - `structure.md`: This file, outlining the project structure.

## Crates (`/crates`)

This directory houses the individual crates that make up the Typforge project.

### `crates/typforge` (Main Application)

*   **Purpose:** The core application crate, responsible for the UI, window management, and orchestrating the other components. It provides the main application shell and integrates all other parts of the project.
*   **Key Features:**
    *   UI rendering and event handling using GPUI.
    *   File management (opening, saving, recent files).
    *   Integration with other crates for editing (`typst-gpui`), previewing (`typst-gpui`), and package management (`package_manager`).
    *   Application menu and keybinding management.
*   **Dependencies:**
    *   **Workspace (Local):**
      *   `gpui_kit` (providing `gpui`, `gpui_base`, and `gpui_component`).
      *   `typst-gpui` (via `typst-gpui = { path = "../typst-gpui" }`).
      *   `typastry` (via `typastry = { path = "../typastry" }`).
      *   `typsdocx` (via `typsdocx = { path = "../typsdocx" }`).
    *   **External:** `chrono`, `parking_lot`, `ropey`, `serde_json`, `anyhow`, `lsp-types`, `embed-resource`, `ureq`, `flate2`, `tar`.
*   **Source Files:**
    *   `Cargo.toml`
    *   `.gitignore`
    *   `src/main.rs`: Entry point of the application.
    *   `src/actions.rs`: Defines application-specific actions (menu items, keybindings).
    *   `src/components/`: UI components and utilities.
        *   `menus.rs`: Handles application menu setup.
        *   `theme.rs`: Manages theme loading and application.
    *   `src/editor/`: Contains the text editing components.
           *   `code_editor.rs`: A reusable, syntax-highlighting code editor component wrapping `EditorState`.
           *   `mod.rs`: Module declaration, exports `EditorStateExt` for UTF-8 byte range manipulation with undo history.
           *   `editor_panel/`: The main panel for editing files, managing `EditorState` entities, tabs, completion providers (`TypstCompletionProvider`), and hover info.
               *   `completions.rs`: Logic for code completion using `typastry`.
               *   `handlers.rs`: Event handlers for the editor panel.
               *   `render.rs`: Renders the editor panel UI.
       *   `src/workspace/`: Top-level application view and layout orchestrator.
           *   Configures `DockArea` with `DockSkin` rendering and `DockLayout` for window splits.
           *   Wraps dock entities using `panel_handle` to bridge `gpui_base::dock::Panel` and `gpui_component::dock::Panel`.
           *   `handlers.rs`: Handles application-level events and actions.
           *   `render.rs`: Renders the main application window UI.
            *   `mod.rs`: Module declaration for the editor panel.
        *   `tabs.rs`: Tab management logic for open files.
    *   `src/key_bindings.rs`: Defines global keybindings.
    *   `src/package_manager/`: UI and logic for managing Typst packages.
        *   `view.rs`: The visual representation of the package manager.
    *   `src/panels/`: Reusable panel components.
        *   `files.rs`: The file explorer panel.
    *   `src/ribbon/`: The UI ribbon/toolbar for formatting and actions.
        *   `panel.rs`: The main ribbon panel component.
        *   `tab_home.rs`: Content for the "Home" tab.
        *   `tab_insert.rs`: Content for the "Insert" tab.
        *   `tab_layout.rs`: Content for the "Page Layout" tab.
        *   `injector.rs`: Helper for converting ribbon actions to edit actions.
    *   `src/settings.rs`: Manages application settings persistence.
    *   `src/typst_world.rs`: Implements the `World` trait for Typst, integrating with the GPUI environment and handling file I/O.
    *   `src/workspace/`: Top-level application view and event handling.
        *   `handlers.rs`: Handles application-level events and actions.
        *   `render.rs`: Renders the main application window UI.

### `crates/typsdocx` (Typst to DOCX Export)

*   **Purpose:** Handles the conversion of Typst document frames into the DOCX format.
*   **Key Features:**
    *   Iterates through Typst frames and page elements.
    *   Translates Typst elements (text, shapes) into DOCX structures using the `docx-rs` crate.
*   **Dependencies:**
    *   **Workspace (Local):**
        *   `typst` (via `typst.workspace`).
        *   `typst-layout` (via `typst-layout.workspace`).
    *   **External:** `docx-rs`, `indexmap`.
*   **Source Files:**
    *   `Cargo.toml`
    *   `.gitignore`
    *   `src/lib.rs`: Main export functionality.

### `crates/typst-gpui` (Typst Rendering for GPUI)

*   **Purpose:** Provides the necessary components and traits to render Typst documents within the GPUI framework. It bridges the gap between Typst's layout engine and GPUI's rendering capabilities, enabling visual preview and interaction.
*   **Key Features:**
    *   Renders Typst `Frame`s into GPUI elements.
    *   Handles text rendering, including glyphs, fonts, and transformations.
    *   Implements the `TypstGpuiWorld` trait, allowing Typst to interact with the GPUI environment for font loading and file access.
    *   Manages canvas interactions, including input handling, caret display, hit testing, and visual edits.
    *   Supports exporting rendered documents to PDF and DOCX.
*   **Dependencies:**
    *   **Workspace (Local):**
        *   `gpui_kit` (providing `gpui` and `gpui_component`).
        *   `typst` (via `typst.workspace`).
        *   `typst-syntax` (via `typst-syntax.workspace`).
        *   `typst-layout` (via `typst-layout.workspace`).
        *   `typsdocx` (via `typsdocx = { path = "../typsdocx" }`).
        *   `typastry` (via `typastry = { path = "../typastry" }`).
    *   **External:** `lyon_path`, `lyon_tessellation`, `ttf-parser`, `png`, `parking_lot`.
*   **Source Files:**
    *   `Cargo.toml`
    *   `.gitignore`
    *   `src/lib.rs`: Core rendering traits and components.
    *   `src/canvas_input.rs`: Canvas input handling and action management.
    *   `src/caret_geometry.rs`: Manages caret positioning and movement logic.
    *   `src/typst_element/`: Rendering logic for specific Typst elements (shapes, text, images, links).

### `crates/typastry` (AST Manipulation and Intelligence)

*   **Purpose:** To provide a high-level, efficient library for manipulating Typst's Abstract Syntax Tree (AST). It aims to reduce boilerplate for editor integrations by offering features like smart editing, autocompletion, and formatting.
*   **Key Features:**
    *   **Smart AST Mutators (`edit` module):** Provides safe, side-effect-free methods for modifying the AST, preventing common nested styling bugs.
    *   **Code Intelligence (`intel` module):** Offers context-aware autocompletion, hover information, and package discovery.
    *   **Direct Formatting (`format` module):** Integrates `typstyle` for high-performance document formatting.
*   **Dependencies:**
    *   **Workspace (Local):**
        *   `typst` (via `typst.workspace`).
        *   `typst-syntax` (via `typst-syntax.workspace`).
        *   `typst-layout` (via `typst-layout.workspace`).
    *   **External:** `serde` (via workspace), `typstyle-core` (optional feature).
*   **Design:** Emphasizes zero-copy operations and microsecond latency by operating directly on AST references where possible, contrasting with typical LSP overhead.
*   **Source Files:**
    *   `Cargo.toml`
    *   `README.md`
    *   `src/lib.rs`: Main library entry point.
    *   `src/edit.rs`: AST editing and text manipulation.
    *   `src/format.rs`: Document formatting.
    *   `src/intel.rs`: Code intelligence (completions, hover).
    *   `src/wysiwyg.rs`: Utilities for WYSIWYG editor interactions.
