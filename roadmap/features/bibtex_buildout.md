# Master Implementation Blueprint: BibTeX Support in TypForge & typASTry

## Context & Vision
We are implementing comprehensive BibTeX citation management and autocompletion for **TypForge** (our native, GPUI-accelerated Typst word processor) and **typASTry** (our custom AST and spatial completion engine). 

We are adopting a strict, incremental, multi-phase roadmap. Prioritize zero-copy performance: reuse existing data structures, lifetime bounds, and pointer wrappers (`Arc`, `Rc`, or borrowed AST nodes) without introducing redundant allocations or cloning. Do not rename existing variables, modify established API surfaces unnecessarily, or break existing completion flows.

---

## Technical Specifications & Architecture Blueprint

### Phase 1: Editor `@` Trigger, Entry Type Completion & Snippet Expansion
*Focus: Editor text-buffer level completion engine.*

1. **Trigger Context & Interception:**
   - Intercept typing when `@` is entered within a bibliography/citation context or inside a `.bib` file scope using `tree-sitter-bibtex` node state.
2. **Entry Type Autocomplete:**
   - Offer completions for standard entry types: `@article`, `@book`, `@inproceedings`, `@phdthesis`, `@mastersthesis`, `@techreport`, `@misc`.
3. **Required-Field Snippet Injection:**
   - Upon selection, insert a formatted snippet with tab-stops. Use `citekey` as the identifier placeholder and include **only required fields**:
     - `@article`: `citekey`, `author`, `title`, `journal`, `year`
     - `@book`: `citekey`, `author`, `title`, `publisher`, `year`
     - `@inproceedings`: `citekey`, `author`, `title`, `booktitle`, `year`
     - `@phdthesis`/`@mastersthesis`: `citekey`, `author`, `title`, `school`, `year`
     - `@techreport`: `citekey`, `author`, `title`, `institution`, `year`
     - `@misc`: `citekey`

### Phase 2: AST Integration & `.bib` Querying in typASTry
*Focus: Parsing, index caching, and zero-clone data extraction.*

1. **AST Node Extraction:**
   - Wire `tree-sitter-bibtex` into `typASTry` to parse `.bib` files into indexed reference maps without duplicating text buffers.
2. **Entry & Field Query API:**
   - Expose fast lookup traits to query existing `citekey`s, missing optional parameters for an entry, and field value spans.
3. **Inline Field Completion:**
   - When typing inside an existing entry block, suggest missing optional fields dynamically based on the current entry type.

### Phase 3: GPUI Bibliography Manager Window & Table View
*Focus: UI overlay, popover/out navigation, and reference indexing.*

1. **Manager UI Layout:**
   - Build a native GPUI modal/popover using standard table components.
   - Display columns: `Citekey` | `Author` | `Title` | `Year` | `Type` | `Actions`.
2. **Data Binding:**
   - Populate the table by querying the indexed `.bib` AST from `typASTry`.
3. **Actions & Search:**
   - Add real-time text filtering across `citekey`, `author`, and `title`.
   - Provide explicit "Add Entry" and "Edit Entry" button triggers.

### Phase 4: Full Entry GUI Form & Clean Re-write Engine
*Focus: Modal forms, dynamic field visibility, and formatting output.*

1. **Add/Edit Modal:**
   - Render form inputs for required fields by default, with an expandable section for optional fields.
   - Pre-fill inputs when editing an existing reference.
2. **Clean Field Serialization:**
   - When saving from the GUI form, perform a full entry rewrite for the targeted block rather than partial patching.
   - **Crucial:** Automatically strip out any empty or whitespace-only `{}` fields on save to ensure clean `.bib` output.
3. **AST Update & Sync:**
   - Write back to the target file and trigger an asynchronous re-parse in `typASTry` to refresh editor completions and table state simultaneously.

---

## Directives for the LLM
1. Read the attached `typASTry` and `TypForge` files before writing code.
2. Maintain zero-copy idioms consistent with our existing architecture.
3. Provide code implementations for each phase sequentially, ensuring all traits, structs, and UI state handlers seamlessly integrate with our codebase.
