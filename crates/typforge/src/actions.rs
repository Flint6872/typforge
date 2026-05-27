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
        // ribbon actions
        ToggleBold,
        ToggleItalic,
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

#[derive(Debug, Clone, PartialEq)]
pub enum RibbonAction {
    // Page Parameters
    SetPaper(String),  // e.g., "us-letter", "a4"
    SetFlipped(bool),  // true = Landscape, false = Portrait
    SetColumns(usize), // e.g., 1 or 2 columns
    SetMargin(String), // e.g., "1in", "2.5cm"

    // Grid Parameters
    InsertGrid { rows: usize, cols: usize },

    // Text Parameters
    SetFont(String),  // e.g., "Liberation Sans", "Linux Libertine"
    SetFontSize(f32), // e.g., 12.0
    ToggleBold,
    ToggleItalic,
    SetTextColor(String), // e.g., "red", "blue", or hex values
}
