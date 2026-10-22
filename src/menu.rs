//! Native menu bar contents. Shortcuts shown here are handled by the same
//! key handler as before; the menu adds discoverability and mouse access.
use crate::commands::{Command, LineCommand};

#[derive(Debug, Clone, Copy)]
pub enum Action {
    Run(Command),
    Palette,
    Copy,
    Cut,
    Paste,
    SelectAll,
    FindNext(bool),
    NextTab,
    PreviousTab,
    MoveToWindow,
    CloseWindow,
}

/// Modifier bits shared with `macos.m`.
pub const CMD: u32 = 1;
pub const SHIFT: u32 = 2;
pub const ALT: u32 = 4;
pub const CTRL: u32 = 8;

pub enum Entry {
    Menu(&'static str),
    /// Title, key equivalent ("up"/"down" for arrows), modifiers, action.
    Item(&'static str, &'static str, u32, Action),
    /// AppKit-provided items, wired to a standard selector in `macos.m`.
    Native(&'static str),
    Separator,
}

use Entry::{Item, Menu, Native, Separator};

pub const MENU: &[Entry] = &[
    Menu("File"),
    Item("New Tab", "n", CMD, Action::Run(Command::New)),
    Item("Open…", "o", CMD, Action::Run(Command::Open)),
    Native("Open Recent"),
    Separator,
    Item("Save", "s", CMD, Action::Run(Command::Save)),
    Item("Save As…", "s", CMD | SHIFT, Action::Run(Command::SaveAs)),
    Separator,
    Item("Move Tab to New Window", "", 0, Action::MoveToWindow),
    Item("Close Tab", "w", CMD, Action::Run(Command::Close)),
    Item("Close Window", "w", CMD | SHIFT, Action::CloseWindow),
    Menu("Edit"),
    Item("Undo", "z", CMD, Action::Run(Command::Undo)),
    Item("Redo", "z", CMD | SHIFT, Action::Run(Command::Redo)),
    Separator,
    Item("Cut", "x", CMD, Action::Cut),
    Item("Copy", "c", CMD, Action::Copy),
    Item("Paste", "v", CMD, Action::Paste),
    Item("Select All", "a", CMD, Action::SelectAll),
    Separator,
    Item("Find…", "f", CMD, Action::Run(Command::Find)),
    Item(
        "Find and Replace…",
        "f",
        CMD | ALT,
        Action::Run(Command::Replace),
    ),
    Item("Find Next", "g", CMD, Action::FindNext(false)),
    Item("Find Previous", "g", CMD | SHIFT, Action::FindNext(true)),
    Separator,
    Item(
        "Duplicate Lines",
        "d",
        CMD | SHIFT,
        Action::Run(Command::Line(LineCommand::Duplicate)),
    ),
    Item(
        "Delete Lines",
        "k",
        CMD | SHIFT,
        Action::Run(Command::Line(LineCommand::Delete)),
    ),
    Item(
        "Move Lines Up",
        "up",
        ALT,
        Action::Run(Command::Line(LineCommand::MoveUp)),
    ),
    Item(
        "Move Lines Down",
        "down",
        ALT,
        Action::Run(Command::Line(LineCommand::MoveDown)),
    ),
    Separator,
    Item("Convert to UTF-8", "", 0, Action::Run(Command::Utf8)),
    Menu("View"),
    Item("Command Palette…", "p", CMD | SHIFT, Action::Palette),
    Separator,
    Item("Split Editor", "\\", CMD, Action::Run(Command::Split)),
    Separator,
    Item("Word Wrap", "w", CMD | ALT, Action::Run(Command::WordWrap)),
    Item("Whitespace", ".", CMD, Action::Run(Command::Whitespace)),
    Item(
        "Syntax Highlighting",
        "",
        0,
        Action::Run(Command::Highlight),
    ),
    Item("Language…", "l", CMD, Action::Run(Command::Syntax)),
    Separator,
    Item("Zoom In", "+", CMD, Action::Run(Command::ZoomIn)),
    Item("Zoom Out", "-", CMD, Action::Run(Command::ZoomOut)),
    Item("Actual Size", "0", CMD, Action::Run(Command::ZoomReset)),
    Separator,
    Native("Enter Full Screen"),
    Menu("Go"),
    Item("Go to Line…", "g", CTRL, Action::Run(Command::Goto)),
    Separator,
    Item("Next Tab", "]", CMD | SHIFT, Action::NextTab),
    Item("Previous Tab", "[", CMD | SHIFT, Action::PreviousTab),
    Menu("Window"),
    Native("Minimize"),
    Native("Zoom"),
    Separator,
    Native("Bring All to Front"),
];
