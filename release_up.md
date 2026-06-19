0.0.2 update
added settings persistence for theme
updated file save dialog to use settings default folder linking back to previous used.

added ribbon to workspace
- Implement `apply_ribbon_action` to handle text formatting and page
  directive updates.
- Include smart toggling for bold/italic and parameter merging in
  `#text` and `#set page` blocks.
- Switch `gpui-component` to local path for development.
- Add ribbon text styling actions and color picker

Implemented bold and italic toggle actions with keyboard shortcuts and
integrated a color picker component into the ribbon panel to allow
changing text color via Typst hex values.

Refactor RibbonAction to return TextEdit object

Introduced a TextEdit struct to encapsulate edit range, new text, and
selection updates. Updated the Ribbon injector to use AST-based logic and
migrated the workspace to use these changes for better history tracking.

Implement font and font size dropdowns in Ribbon panel

Add interactive popover menus for font selection and size configuration,
sourcing available fonts from the Typst world and managing state
transitions with preview reverts.

Initialize typforge-core and remove LSP dependency

Create `typforge-core` for native AST manipulation, IDE features, and
formatting. Remove the `typstography` crate and redundant LSP client
infrastructure, migrating core logic into the new workspace member.

Implement diagnostics display and autocomplete UI

- Render Typst compilation diagnostics as code editor underlines
- Add popup UI for autocomplete suggestions
- Propagate diagnostic events from the preview panel to the editor

Refactor completion logic to use standard CompletionProvider

- Introduce `TypstCompletionProvider` implementation
- Remove custom completion rendering from `EditorPanel`
- Add `lsp-types` dependency to handle completion standard types

Refine Typst completion logic and cursor behavior

- Update completion triggers to support identifiers and delimiters
- Normalize snippet text to plain text to prevent formatting issues
- Implement auto-jump feature to move cursor inside parentheses after
  function completion

Implement dynamic size completions and ribbon selection tracking

- Add alphanumeric size coaching to completion provider
- Implement `detect_properties_at_offset` to track styling properties
- Add `EditorSelectionChanged` event and update ribbon state in real-time

Implement file and source caching in GpuiWorld

Add thread-safe caching with rate-limited disk access for `GpuiWorld` to
improve performance. Also increment the project version.

Add Deep Dark: Minecraft Story (AI generated) document and font management guide

- Added "Deep Dark" story structure and chapters.
- Added Typst font management and fallback reference guide.

Update document metadata before updating source

Ensure metadata and root folder are set prior to compilation to guarantee 
correct resolution of relative paths and include files. Also remove the 
length check to prevent compilation failures when switching between files 
of identical length.

Optimize editor performance and compilation logic

- Replace linear scan with nested binary search in `screen_to_byte_offset`
- Implement debounced, asynchronous background compilation in `PreviewPanel`
