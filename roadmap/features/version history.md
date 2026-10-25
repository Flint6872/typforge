I am building a local-first, GPUI-powered desktop word processor for Typst projects in Rust. I want to implement local version history, multi-file workspace management, real-time file-sync collaboration (via SharePoint/network shares), and a review workflow for training document drafts. 

Before we write any new implementation code, please follow a strict phased approach. Do not jump ahead to the final code all at once. Complete each phase, verify it, and wait for my confirmation before moving to the next.

---

### Phase 0: Codebase Audit & Setup Review
- **Objective:** Review my current Rust project setup, existing crates (including GPUI and Typst bindings), and file structure to identify what code already exists.
- **Action:** Ask me to paste or describe my current project structure, relevant structs, and main entry points. Analyze them so we don't duplicate existing work or conflict with our architectural choices.
- **Milestone:** Summarize an integration plan tailored specifically to my current codebase.

---

### Phase 1: Loro CRDT Integration & Workspace State
- **Objective:** Integrate the `loro` Rust crate to manage a multi-file project workspace.
- **Architecture:** 
  - The project folder contains loose source files (`*.typ`, `*.bib`) and assets (images/fonts as loose files).
  - A hidden project-level sidecar state file manages a unified map/tree where keys are file paths and values are Loro Text containers.
- **Action:** Implement the core project state struct, file-system loading/saving routines for the sidecar, and basic initialization.

---

### Phase 2: In-Memory Typst `World` Trait & Network File Sync
- **Objective:** Implement Typst's `World` trait using in-memory buffers and configure resilient file-sync.
- **Architecture:**
  - **Typst World Trait:** Implement Typst's compilation world by reading directly from Loro's in-memory text buffers in RAM for zero-latency live previews, with background tasks periodically flushing changes to physical `.typ` files.
  - **SharePoint / Network Shares:** Configure background save routines with **automatic retries and exponential backoff** to handle SMB/network file-locking gracefully without throwing permission errors to the user.
- **Action:** Write the custom `World` trait implementation and the robust retry/file-saving wrapper.

---

### Phase 3: External Edit Reconciliation (Side-by-Side Diff)
- **Objective:** Handle cases where a user modifies a `.typ` file externally (e.g., quick edit in Notepad) while the app is closed.
- **Architecture:** 
  - On app startup, compare the modification timestamp/content of the physical `.typ` file against the Loro state.
  - If a mismatch is found, present a side-by-side diff UI allowing the user to choose: Keep Disk Version, Keep Project History, or Merge.
- **Action:** Implement startup detection logic and the reconciliation diff flow.

---

### Phase 4: Draft Branching & Review Panel ("Requests to Change")
- **Objective:** Implement local workspace branching for training document updates and a review workflow.
- **Architecture:**
  - Leverage Loro's `Frontiers` to fork the project state into a draft branch (e.g., `[Draft: Training Update]`).
  - Build a review interface/panel that computes diffs between `main` and the active draft across all project files.
  - Provide inline [Accept] / [Reject] controls for reviewers to curate updates before merging the branch back into `main`.
- **Action:** Implement branch creation, diff calculation across Loro containers, and the review panel UI logic.

---

Please acknowledge this roadmap, confirm you understand the architecture, and begin with **Phase 0** by asking to review my current setup and code.
