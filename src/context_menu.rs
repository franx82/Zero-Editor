use crate::{Message, commands::Command, theme};
use iced::widget::{button, column, container, row, text};
use iced::{Element, Length, Point};

#[derive(Debug, Clone)]
pub enum Target {
    Editor(String),
    Tab(String),
    Empty,
}
#[derive(Debug, Clone)]
pub struct Menu {
    pub target: Target,
    pub position: Point,
    pub selected: Option<usize>,
}
#[derive(Debug, Clone, Copy)]
pub enum Action {
    Command(Command),
    Copy,
    Cut,
    Paste,
    SelectAll,
    CopyPath,
    Split,
    Detach,
}

pub type Entry = (&'static str, &'static str, Action, bool);

pub fn entries(menu: &Menu, tab: &crate::Tab, has_neighbor: bool) -> Vec<Entry> {
    let has_selection = tab.content.selection().is_some();
    let has_path = tab.path.is_some();
    match &menu.target {
        Target::Editor(_) => vec![
            ("Undo", "⌘Z", Action::Command(Command::Undo), tab.can_undo()),
            (
                "Redo",
                "⌘⇧Z",
                Action::Command(Command::Redo),
                tab.can_redo(),
            ),
            ("Cut", "⌘X", Action::Cut, has_selection),
            ("Copy", "⌘C", Action::Copy, has_selection),
            ("Paste", "⌘V", Action::Paste, true),
            ("Select all", "⌘A", Action::SelectAll, true),
            ("Find…", "⌘F", Action::Command(Command::Find), true),
            (
                "Find and replace…",
                "⌘⌥F",
                Action::Command(Command::Replace),
                true,
            ),
        ],
        Target::Tab(_) => vec![
            ("Save", "⌘S", Action::Command(Command::Save), true),
            ("Save as…", "⌘⇧S", Action::Command(Command::SaveAs), true),
            ("Copy path", "", Action::CopyPath, has_path),
            (
                "Split with adjacent tab",
                "⌘\\",
                Action::Split,
                has_neighbor,
            ),
            ("Move to new window", "", Action::Detach, true),
            ("Close tab", "⌘W", Action::Command(Command::Close), true),
        ],
        Target::Empty => vec![
            ("New document", "⌘N", Action::Command(Command::New), true),
            ("Open file…", "⌘O", Action::Command(Command::Open), true),
        ],
    }
}

pub fn view(menu: &Menu, tab: &crate::Tab, has_neighbor: bool) -> Element<'static, Message> {
    let items = entries(menu, tab, has_neighbor);
    let mut content = column![].spacing(2);
    for (index, (label, hint, action, enabled)) in items.into_iter().enumerate() {
        let selected = menu.selected == Some(index);
        content = content.push(
            button(
                row![
                    text(label).size(theme::TEXT_MD).width(Length::Fill),
                    text(hint).size(theme::TEXT_XS).color(theme::TEXT_MUTED)
                ]
                .spacing(12),
            )
            .on_press_maybe(enabled.then_some(Message::ContextAction(action)))
            .padding([8, 10])
            .width(Length::Fill)
            .style(if selected && enabled {
                theme::selected_item_style
            } else {
                theme::picker_item_style
            }),
        );
    }
    container(content)
        .width(290)
        .padding(6)
        .style(theme::picker_container_style)
        .into()
}
