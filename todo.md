
## To-Do List

*  [ ] fix having to double click on a file with #include to show in preview

### `typsdocx` (Export to Word *.docx file)
Post TypForge 0.1.0
* [ ] translate typst through docx-rs to export to word *.docx.

### `typst-gpui` (Preview Panel)
* [X] export to pdf using typst-pdf.
* [ ] setup package manager using typst-kit (questionable to have here or main crate)

#### elements
* [ ] #text (Not Rotating)
* [ ] #rect
* [ ] #block
* [ ] #circle 
* [ ] #curve
* [ ] #ellipse - showing as pill shape 
* [ ] #line
* [ ] #polygon
* [ ] #square

Issues color.map not working on non-square objects
To get true multi-stop radial gradients or rainbow circles in GPUI, we would eventually need to implement a **Triangle Fan Tessellator** or a **Custom Shader**, which are much deeper tasks. This fix gets your basic visualization looking correct and consistent.

- `typst features` 
  - * [X] #include 
  - * [X] fill: gradient.linear( ..color.map.viridis,),


### `typforge` (Main Application)
*   [X] Lazily load folders in the files panel to improve performance and responsiveness, especially for large projects.

*   [ ] figure out how to add Typst lsp for pub fn _set_language().
*  [ ] fix when saving a new file to be save file as
*  [ ] inactiveate menu itmes when not available (example.. Export when no file open)
*   [ ] figure out Distribution Strategy Use cargo-dist in your CI/CD. It can - - - 
  - automatically generate:
    - Windows installers (.msi).
    - macOS bundles (.dmg).
    - Shell script installers (curl | sh) for Linux/macOS
    

*  [ ] build out FilePanel to mirror Zed's ability for right click and drag and drop
*  [ ] Menu -> View: build out ablity to show/hide/pop panels

* [ ] ablitiy to save to Git for version history. (possibly using [https://github.com/gitoxidelabs/gitoxide])

## Completed Items

### `typforge` - Files Panel & Editor Integration

*   The foundational structure for the `FilesPanel` and `EditorPanel` has been laid out, with initial plans for displaying project structure and handling file opening.
    *   Refer to: `typforge/crates/typforge/.zed/files_project_outline.md` for the detailed project outline.
*   The basic file system scanning and tree display for the `FilesPanel` is being implemented.
*   The `EditorPanel` is being set up to manage multiple open files in a tabbed interface.
*   An event mechanism for opening files from the `FilesPanel` to the `EditorPanel` is in progress.

### `typforge` - Menu Bar & Window Controls

*   A clear pattern for defining actions, key bindings, and integrating them into a `MenuBar` component has been established.
    *   Refer to: `typforge/crates/typforge/.zed/menubar_rules.md` for implementation guidelines.
*   The structure for a custom title bar, including draggable areas and window control buttons (minimize, maximize, close), is defined.

### `typst-gpui` - Core Rendering & Library Structure

*   The `typst-gpui` crate is being refactored into a library to be consumed by `typforge`.
    *   Refer to: `typforge/crates/typst-gpui/.zed/preview_project.md` for the integration plan.
*   The core rendering logic for Typst documents within GPUI views is being defined, including coordinate system mappings and font synchronization.
    *   Refer to: `typforge/crates/typst-gpui/.zed/rules.md` for detailed API and coordinate system rules.
    *   Refer to: `typforge/crates/typst-gpui/.zed/project.md` for the broader project objective and roadmap.
