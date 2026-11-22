I am building a pure Rust, offline-first Typst word processor using GPUI to ensure a low memory footprint and a responsive, lag-free UI. 

Here is my Cargo workspace layout:
- `crates/typforge` — main binary
- `crates/typsdocx` — Typst to MS Word export
- `crates/typst-gpui` — GPUI renderer, text/glyph rendering, caret, hit testing, canvas interactions, PDF/DOCX export (depends on `gpui_kit`, `typst`, `typst-syntax`, `typst-layout`, `typsdocx`, `typastry`)
- `crates/typastry` — holds AST info; designed to be reusable/independent for other Typst editors

I am considering placing AST-aware spell/grammar diagnostic logic in `typastry` (potentially behind a feature flag so external consumers aren't forced to take it) while keeping GPUI background execution, hit-map lookup, and right-click menu handling in `typst-gpui` (initially targeting rendered/preview interaction, with clean span mapping so it can extend to editor text as well). please review current code to help me decide where it would best fit.

### Architecture & Requirements:
1. **Libraries:** Use `zspell` (Hunspell dictionaries) for spell checking and `harper-core` for grammar/style checking.
2. **Concurrency:** Use **GPUI's built-in background executor** (`cx.background_executor()`) to run checks asynchronously without dropping UI frames.
3. **Dictionaries:** Load dictionary files from a local directory. Detect language based on Typst document configuration (e.g., `#set text(lang: "...")`), falling back to English if the specific language file does not exist. Support user/project custom ignore lists.
4. **AST Boundary Handling:** Skip checking Typst-specific code blocks, math environments ($...$), control sequences (#...), and label definitions, ensuring byte ranges map accurately back to the raw source text buffer.
5. **Unified Diagnostics:** Normalize outputs from both libraries into a unified diagnostic struct containing byte ranges, error descriptions, and correction suggestions.
6. **UI Integration (Right-Click Context Menu):** Update right-click handling to query my existing **hit map**, retrieve diagnostics at the cursor position, and pop up a context menu showing:
   - Spelling suggestions at the top (if any)
   - "Inspect Element" (for your existing introspection feature)
   - "Format Selection"
   - Standard text options (Copy, Paste, Select All)

Please provide a phased, implementation-ready guide and idiomatic Rust code starting with setting up the diagnostic structures, dictionary loader, and background worker using GPUI's background executor.
