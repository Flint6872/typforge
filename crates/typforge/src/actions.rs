use gpui::actions;
use serde::Deserialize;

// Define actions for your application's menu items.
// Using a specific namespace for your app (e.g., `app_actions`).
actions!(
    menu_ui,
    [
        FileNew,
        FileOpen,
        FileSave,
        FileSaveAs,
        FileClose,
        FileQuit,
        FolderOpen,
        EditUndo,
        EditRedo,
        EditCut,
        EditCopy,
        EditPaste,
        ViewToggleSidebar,
        HelpAbout,
        ZoomIn,
        ZoomOut,
        ResetZoom,
        FileExportPdf,
        FileExportDocx,
        PackageManager,
        ReloadSettings,
        // ChangeTheme,
    ]
);

#[derive(gpui::Action, Clone, PartialEq, Eq, Deserialize)]
#[action(namespace = actions, no_json)]
pub struct ChangeTheme {
    pub name: String,
}

// You can also define custom action structs if they need parameters.
// For example, if you wanted a "Go to Line" action:
// #[derive(Action, Clone, PartialEq, Eq, Deserialize)]
// #[action(namespace = app_actions, no_json)]
// pub struct GoToLine {
//     pub line_number: usize,
// }
