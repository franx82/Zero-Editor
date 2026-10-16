#[derive(Debug, Clone, Copy)]
pub enum LineCommand {
    Duplicate,
    Delete,
    MoveUp,
    MoveDown,
}

#[derive(Debug, Clone, Copy)]
pub enum Command {
    New,
    Open,
    Save,
    SaveAs,
    Close,
    Find,
    Replace,
    Goto,
    Undo,
    Redo,
    Line(LineCommand),
    Split,
    Syntax,
    Highlight,
    Whitespace,
    Utf8,
    ZoomIn,
    ZoomOut,
    ZoomReset,
}

pub const COMMANDS: &[(Command, &str, &str)] = &[
    (Command::New, "New file", "⌘N"),
    (Command::Open, "Open file…", "⌘O"),
    (Command::Save, "Save", "⌘S"),
    (Command::SaveAs, "Save as…", "⌘⇧S"),
    (Command::Close, "Close tab", "⌘W"),
    (Command::Find, "Find in file", "⌘F"),
    (Command::Replace, "Find and replace", "⌘⌥F"),
    (Command::Goto, "Go to line:column…", "⌃G"),
    (Command::Undo, "Undo", "⌘Z"),
    (Command::Redo, "Redo", "⌘⇧Z"),
    (
        Command::Line(LineCommand::Duplicate),
        "Duplicate line or selected lines",
        "⌘⇧D",
    ),
    (
        Command::Line(LineCommand::Delete),
        "Delete line or selected lines",
        "⌘⇧K",
    ),
    (Command::Line(LineCommand::MoveUp), "Move lines up", "⌥↑"),
    (
        Command::Line(LineCommand::MoveDown),
        "Move lines down",
        "⌥↓",
    ),
    (Command::Split, "Toggle split with adjacent tab", "⌘\\"),
    (Command::Syntax, "Select language…", "⌘L"),
    (Command::Highlight, "Toggle syntax highlighting", ""),
    (Command::Whitespace, "Toggle whitespace markers", "⌘."),
    (Command::Utf8, "Convert document to UTF-8", ""),
    (Command::ZoomIn, "Increase text size", "⌘+"),
    (Command::ZoomOut, "Decrease text size", "⌘−"),
    (Command::ZoomReset, "Reset text size", "⌘0"),
];

pub fn filtered(query: &str) -> Vec<usize> {
    let terms: Vec<String> = query.split_whitespace().map(str::to_lowercase).collect();
    COMMANDS
        .iter()
        .enumerate()
        .filter(|(_, (_, label, _))| {
            let label = label.to_lowercase();
            terms.iter().all(|term| label.contains(term))
        })
        .map(|(i, _)| i)
        .collect()
}
