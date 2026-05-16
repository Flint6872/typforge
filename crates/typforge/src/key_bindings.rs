use crate::actions;
use gpui::{App, KeyBinding}; // Import your application actions

pub fn bind_keys(cx: &mut App) {
    const GLOBAL_CONTEXT: Option<&str> = None; // Global context for menu actions

    let mut bindings = Vec::new();

    // --- Application-specific Global Keybindings ---
    #[cfg(target_os = "macos")]
    bindings.extend([
        KeyBinding::new("cmd-n", actions::FileNew, GLOBAL_CONTEXT),
        KeyBinding::new("cmd-o", actions::FileOpen, GLOBAL_CONTEXT),
        KeyBinding::new("cmd-s", actions::FileSave, GLOBAL_CONTEXT),
        KeyBinding::new("cmd-shift-s", actions::FileSaveAs, GLOBAL_CONTEXT),
        KeyBinding::new("cmd-w", actions::FileClose, GLOBAL_CONTEXT),
        KeyBinding::new("cmd-q", actions::FileQuit, GLOBAL_CONTEXT),
        KeyBinding::new("cmd-z", actions::EditUndo, GLOBAL_CONTEXT),
        KeyBinding::new("cmd-shift-z", actions::EditRedo, GLOBAL_CONTEXT),
        KeyBinding::new("cmd-x", actions::EditCut, GLOBAL_CONTEXT),
        KeyBinding::new("cmd-c", actions::EditCopy, GLOBAL_CONTEXT),
        KeyBinding::new("cmd-v", actions::EditPaste, GLOBAL_CONTEXT),
        // Add more Mac specific global keybindings here
    ]);

    #[cfg(not(target_os = "macos"))] // Windows / Linux
    bindings.extend([
        KeyBinding::new("ctrl-n", actions::FileNew, GLOBAL_CONTEXT),
        KeyBinding::new("ctrl-o", actions::FileOpen, GLOBAL_CONTEXT),
        KeyBinding::new("ctrl-k", actions::FolderOpen, GLOBAL_CONTEXT),
        KeyBinding::new("ctrl-s", actions::FileSave, GLOBAL_CONTEXT),
        KeyBinding::new("ctrl-shift-s", actions::FileSaveAs, GLOBAL_CONTEXT),
        KeyBinding::new("ctrl-w", actions::FileClose, GLOBAL_CONTEXT),
        KeyBinding::new("ctrl-q", actions::FileQuit, GLOBAL_CONTEXT),
        KeyBinding::new("ctrl-z", actions::EditUndo, GLOBAL_CONTEXT),
        KeyBinding::new("ctrl-y", actions::EditRedo, GLOBAL_CONTEXT), // Ctrl-Y is common for Redo on Win/Linux
        KeyBinding::new("ctrl-x", actions::EditCut, GLOBAL_CONTEXT),
        KeyBinding::new("ctrl-c", actions::EditCopy, GLOBAL_CONTEXT),
        KeyBinding::new("ctrl-v", actions::EditPaste, GLOBAL_CONTEXT),
        KeyBinding::new("ctrl-=", actions::ZoomIn, GLOBAL_CONTEXT),
        KeyBinding::new("ctrl-+", actions::ZoomIn, GLOBAL_CONTEXT), // ctrl-+ is often ctrl-shift-=
        KeyBinding::new("ctrl--", actions::ZoomOut, GLOBAL_CONTEXT),
        KeyBinding::new("ctrl-0", actions::ResetZoom, GLOBAL_CONTEXT),
        // Add more Windows/Linux specific global keybindings here
    ]);

    cx.bind_keys(bindings);
}
