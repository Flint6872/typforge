use crate::actions::RibbonAction;
use typastry::edit::EditAction;

impl From<RibbonAction> for EditAction {
    fn from(action: RibbonAction) -> Self {
        match action {
            RibbonAction::ToggleBold => EditAction::ToggleBold,
            RibbonAction::ToggleItalic => EditAction::ToggleItalic,
            RibbonAction::ToggleUnderline => EditAction::ToggleUnderline,
            RibbonAction::SetFont(f) => EditAction::SetFont(f),
            RibbonAction::SetFontSize(s) => EditAction::SetFontSize(s as f64),
            RibbonAction::SetTextColor(c) => EditAction::SetTextColor(c),
            RibbonAction::InsertGrid { rows, cols } => EditAction::InsertGrid { rows, cols },
            RibbonAction::SetPaper(p) => EditAction::SetPaper(p),
            RibbonAction::SetFlipped(f) => EditAction::SetFlipped(f),
            RibbonAction::SetColumns(c) => EditAction::SetColumns(c),
            RibbonAction::SetMargin(m) => EditAction::SetMargin(m),
        }
    }
}

impl From<&RibbonAction> for EditAction {
    fn from(action: &RibbonAction) -> Self {
        action.clone().into()
    }
}
