mod commands;
mod context_menu;
mod desktop;
mod gutter;
mod highlighter;
#[cfg(target_os = "macos")]
mod macos;
mod scrollbar;
mod search;
mod session;
mod storage;
mod tab;
mod tab_bar;
mod theme;
mod typography;

use commands::{COMMANDS, Command, LineCommand};
use iced::widget::{
    button, canvas, column, container, mouse_area, operation, pin, responsive, row, space, stack,
    text, text_editor, text_input,
};
use iced::{
    Alignment, Element, Length, Padding, Task,
    keyboard::{self, Key},
};
use std::path::PathBuf;
use tab::Tab;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Panel {
    None,
    Find,
    Replace,
    Palette,
    Goto,
    Language,
}

struct App {
    tabs: Vec<Tab>,
    active_tab: usize,
    highlight_enabled: bool,
    show_whitespace: bool,
    untitled_counter: usize,
    status_notification: Option<String>,
    panel: Panel,
    panel_query: String,
    panel_index: usize,
    search: search::Search,
    font_size: f32,
    close_confirmation: Option<String>,
    close_after_save: Option<String>,
    revision: u64,
    session_blocked: bool,
    ui_id: String,
    split_anchor: Option<String>,
    context: Option<context_menu::Menu>,
    pointer: iced::Point,
}

#[derive(Debug, Clone)]
enum Message {
    Quit,
    KeyPressed(Key, keyboard::Modifiers, bool),
    Edit(text_editor::Action),
    Indent,
    Unindent,
    SmartBackspace,
    SelectTab(usize),
    CloseTab(usize),
    NextTab,
    PreviousTab,
    Run(Command),
    OpenFileDialog,
    FilePicked(Option<PathBuf>),
    FileDropped(PathBuf),
    SaveAsPicked(String, Option<PathBuf>),
    ShowPanel(Panel),
    ClosePanel,
    PanelQuery(String),
    PanelStep(bool),
    PanelSubmit,
    SelectLanguage(String),
    SearchQuery(String),
    Replacement(String),
    FindNext(bool),
    ToggleCase,
    ToggleRegex,
    ReplaceOne,
    ReplaceAll,
    CancelClose,
    DiscardClose,
    SaveClose,
    DismissNotification,
    Tabs(tab_bar::Intent),
    ToggleSplit,
    DetachTab(String),
    EditAt(String, text_editor::Action),
    ForTab(String, Box<Message>),
    ViewportAt(String, f32),
    ScrollAt(String, usize),
    OpenEditorContext(String),
    ContextAction(context_menu::Action),
    PasteText(String, Option<String>),
    FocusSearchInput(bool),
}

fn expand_tilde(path: PathBuf) -> PathBuf {
    if let Ok(relative) = path.strip_prefix("~")
        && let Some(home) = dirs::home_dir()
    {
        return home.join(relative);
    }
    path
}

impl App {
    fn from_tabs(tabs: Vec<Tab>, active_tab: usize, status_notification: Option<String>) -> Self {
        Self {
            tabs,
            active_tab,
            highlight_enabled: true,
            show_whitespace: false,
            untitled_counter: 0,
            status_notification,
            panel: Panel::None,
            panel_query: String::new(),
            panel_index: 0,
            search: search::Search::default(),
            font_size: 15.0,
            close_confirmation: None,
            close_after_save: None,
            revision: 0,
            session_blocked: false,
            ui_id: storage::unique_id(),
            split_anchor: None,
            context: None,
            pointer: iced::Point::ORIGIN,
        }
    }

    fn fresh_tab(&mut self) -> Tab {
        loop {
            let tab = Tab::new_empty(self.untitled_counter);
            self.untitled_counter += 1;
            if self.tabs.iter().all(|existing| existing.title != tab.title) {
                return tab;
            }
        }
    }

    fn editor_id(&self, id: &str) -> String {
        format!("editor-{id}")
    }
    fn panel_id(&self) -> String {
        format!("panel-{}", self.ui_id)
    }
    fn replacement_id(&self) -> String {
        format!("replace-{}", self.ui_id)
    }
    fn focus_editor(&self) -> Task<Message> {
        operation::focus(self.editor_id(&self.tabs[self.active_tab].id))
    }
    fn unfocus_editor(&self) -> Task<Message> {
        operation::focus(format!("inactive-{}", self.ui_id))
    }

    fn focus_panel(&self) -> Task<Message> {
        operation::focus(self.panel_id())
    }
    fn split_pair(&self) -> Option<(usize, usize)> {
        let left = self
            .tabs
            .iter()
            .position(|t| Some(&t.id) == self.split_anchor.as_ref())?;
        (left + 1 < self.tabs.len()).then_some((left, left + 1))
    }
    fn normalize_split(&mut self) {
        if self.split_pair().is_none() {
            self.split_anchor = None;
        }
    }
    fn adjacent_split_anchor(&self) -> Option<String> {
        if self.tabs.len() < 2 {
            return None;
        }
        Some(
            self.tabs[self.active_tab.min(self.tabs.len() - 2)]
                .id
                .clone(),
        )
    }

    fn title(&self) -> String {
        let tab = &self.tabs[self.active_tab];
        format!(
            "{}{} — Zero Editor",
            tab.title,
            if tab.is_dirty { " •" } else { "" }
        )
    }

    fn changed(&mut self) {
        self.revision += 1;
    }

    fn refresh_search(&mut self) {
        self.search
            .refresh(&self.tabs[self.active_tab].content.text());
    }

    fn find(&mut self, backwards: bool) {
        let tab = &mut self.tabs[self.active_tab];
        let text = tab.content.text();
        let offset = search::offset_at(&text, tab.content.cursor().position);
        if let Some(range) = self.search.next(offset, backwards) {
            tab.select_range(range.start, range.end);
            self.changed();
        }
    }

    fn remove_tab(&mut self, index: usize) {
        let old_active = self.active_tab;
        self.tabs.remove(index);
        if self.tabs.is_empty() {
            let tab = self.fresh_tab();
            self.tabs.push(tab);
            self.active_tab = 0;
        } else if index < old_active {
            self.active_tab -= 1;
        } else {
            self.active_tab = self.active_tab.min(self.tabs.len() - 1);
        }
        self.close_confirmation = None;
        self.normalize_split();
        self.changed();
        self.refresh_search();
    }

    fn save_tab(&mut self, id: String, path: Option<PathBuf>) -> Task<Message> {
        let Some(index) = self.tabs.iter().position(|t| t.id == id) else {
            return self.focus_editor();
        };
        let result = if let Some(path) = path {
            self.tabs[index].save_as(path)
        } else {
            self.tabs[index].save()
        };
        match result {
            Ok(()) => {
                self.status_notification = Some("File saved".into());
                if self.close_after_save.as_deref() == Some(&id) {
                    self.remove_tab(index);
                    self.close_after_save = None;
                }
                self.changed();
            }
            Err(e) => {
                self.close_after_save = None;
                self.status_notification = Some(format!("Could not save file: {e}"));
            }
        }
        self.focus_editor()
    }

    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::Tabs(intent) => {
                use tab_bar::Intent;
                match intent {
                    Intent::Select(index) => return self.update(Message::SelectTab(index)),
                    Intent::Close(index) => return self.update(Message::CloseTab(index)),
                    Intent::New => return self.update(Message::Run(Command::New)),
                    Intent::Split => return self.update(Message::ToggleSplit),
                    Intent::Palette => return self.update(Message::ShowPanel(Panel::Palette)),
                    Intent::Detach(id) => return Task::done(Message::DetachTab(id)),
                    Intent::Reorder { id, slot } => {
                        if let Some(from) = self.tabs.iter().position(|t| t.id == id) {
                            let active = self.tabs[self.active_tab].id.clone();
                            let tab = self.tabs.remove(from);
                            let to = slot
                                .saturating_sub(usize::from(from < slot))
                                .min(self.tabs.len());
                            self.tabs.insert(to, tab);
                            self.active_tab =
                                self.tabs.iter().position(|t| t.id == active).unwrap_or(0);
                            if self.split_anchor.is_some() {
                                self.split_anchor = self.adjacent_split_anchor();
                                self.normalize_split();
                            }
                            self.changed();
                        }
                        return self.focus_editor();
                    }
                    Intent::Context(id, position) => {
                        if let Some(id) = id {
                            if let Some(index) = self.tabs.iter().position(|t| t.id == id) {
                                let _ = self.update(Message::SelectTab(index));
                            }
                            self.context = Some(context_menu::Menu {
                                target: context_menu::Target::Tab(id),
                                position,
                                selected: None,
                            });
                        } else {
                            self.context = Some(context_menu::Menu {
                                target: context_menu::Target::Empty,
                                position,
                                selected: None,
                            });
                        }
                        self.panel = Panel::None;
                        return self.unfocus_editor();
                    }
                }
            }
            Message::ToggleSplit => {
                self.context = None;
                self.panel = Panel::None;
                if self.split_anchor.is_some() {
                    self.split_anchor = None;
                } else {
                    self.split_anchor = self.adjacent_split_anchor();
                }
                return self.focus_editor();
            }
            Message::EditAt(id, action) => {
                if let Some(index) = self.tabs.iter().position(|t| t.id == id) {
                    if matches!(action, text_editor::Action::Scroll { .. }) {
                        self.tabs[index].on_edit(action);
                    } else {
                        if self.active_tab != index {
                            self.active_tab = index;
                            self.refresh_search();
                        }
                        return self.update(Message::Edit(action));
                    }
                }
            }
            Message::ForTab(id, message) => {
                if let Some(index) = self.tabs.iter().position(|t| t.id == id) {
                    self.active_tab = index;
                    return self.update(*message);
                }
            }
            Message::ViewportAt(id, height) => {
                if let Some(tab) = self.tabs.iter_mut().find(|t| t.id == id) {
                    tab.set_viewport(height, self.font_size * 1.4);
                }
            }
            Message::ScrollAt(id, line) => {
                if let Some(tab) = self.tabs.iter_mut().find(|t| t.id == id) {
                    tab.scroll_to_line(line);
                }
            }
            Message::OpenEditorContext(id) => {
                if let Some(index) = self.tabs.iter().position(|t| t.id == id) {
                    self.active_tab = index;
                }
                self.panel = Panel::None;
                self.context = Some(context_menu::Menu {
                    target: context_menu::Target::Editor(id),
                    position: self.pointer,
                    selected: None,
                });
                return self.unfocus_editor();
            }
            Message::ContextAction(action) => {
                let Some(menu) = self.context.take() else {
                    return Task::none();
                };
                if let context_menu::Target::Tab(id) | context_menu::Target::Editor(id) =
                    menu.target
                {
                    if let Some(index) = self.tabs.iter().position(|t| t.id == id) {
                        self.active_tab = index;
                    } else {
                        return Task::none();
                    }
                }
                use context_menu::Action;
                match action {
                    Action::Command(command) => return self.update(Message::Run(command)),
                    Action::Copy => {
                        if let Some(text) = self.tabs[self.active_tab].content.selection() {
                            return iced::clipboard::write(text).chain(self.focus_editor());
                        }
                    }
                    Action::Cut => {
                        if let Some(text) = self.tabs[self.active_tab].content.selection() {
                            let id = self.tabs[self.active_tab].id.clone();
                            let task = self.update(Message::EditAt(
                                id,
                                text_editor::Action::Edit(text_editor::Edit::Backspace),
                            ));
                            return Task::batch([
                                iced::clipboard::write(text),
                                task,
                                self.focus_editor(),
                            ]);
                        }
                    }
                    Action::Paste => {
                        let id = self.tabs[self.active_tab].id.clone();
                        return iced::clipboard::read()
                            .map(move |text| Message::PasteText(id.clone(), text));
                    }
                    Action::SelectAll => {
                        self.tabs[self.active_tab]
                            .content
                            .perform(text_editor::Action::SelectAll);
                        self.changed();
                        return self.focus_editor();
                    }
                    Action::CopyPath => {
                        if let Some(path) = &self.tabs[self.active_tab].path {
                            return iced::clipboard::write(path.display().to_string())
                                .chain(self.focus_editor());
                        }
                    }
                    Action::Split => {
                        self.split_anchor = None;
                        return self.update(Message::ToggleSplit);
                    }
                    Action::Detach => {
                        return Task::done(Message::DetachTab(
                            self.tabs[self.active_tab].id.clone(),
                        ));
                    }
                }
                return self.focus_editor();
            }
            Message::PasteText(id, text) => {
                if let Some(text) = text {
                    let _ = self.update(Message::EditAt(
                        id,
                        text_editor::Action::Edit(text_editor::Edit::Paste(std::sync::Arc::new(
                            text,
                        ))),
                    ));
                }
                return self.focus_editor();
            }
            Message::FocusSearchInput(query_was_focused) => {
                return operation::focus(if query_was_focused {
                    self.replacement_id()
                } else {
                    self.panel_id()
                });
            }

            Message::KeyPressed(key, modifiers, captured) => {
                if let Some(menu) = self.context.as_mut() {
                    use keyboard::key::Named;
                    let entries = context_menu::entries(
                        menu,
                        &self.tabs[self.active_tab],
                        self.tabs.len() > 1,
                    );
                    match key {
                        Key::Named(Named::Escape) => return self.update(Message::ClosePanel),
                        Key::Named(Named::Enter) => {
                            if let Some(index) = menu.selected
                                && entries[index].3
                            {
                                return self.update(Message::ContextAction(entries[index].2));
                            }
                        }
                        Key::Named(Named::ArrowUp) | Key::Named(Named::ArrowDown) => {
                            let enabled: Vec<usize> = entries
                                .iter()
                                .enumerate()
                                .filter(|(_, e)| e.3)
                                .map(|(i, _)| i)
                                .collect();
                            if !enabled.is_empty() {
                                let at = menu
                                    .selected
                                    .and_then(|s| enabled.iter().position(|i| *i == s));
                                let next = if key == Key::Named(Named::ArrowUp) {
                                    at.map(|i| (i + enabled.len() - 1) % enabled.len())
                                        .unwrap_or(enabled.len() - 1)
                                } else {
                                    at.map(|i| (i + 1) % enabled.len()).unwrap_or(0)
                                };
                                menu.selected = Some(enabled[next]);
                            }
                        }
                        _ => {}
                    }
                    return Task::none();
                }

                if !captured
                    && self.panel == Panel::Replace
                    && key == Key::Named(keyboard::key::Named::Tab)
                {
                    return operation::is_focused(self.panel_id()).map(Message::FocusSearchInput);
                }
                if self.close_confirmation.is_some() {
                    if key == Key::Named(keyboard::key::Named::Escape) {
                        return self.update(Message::CancelClose);
                    }
                    if key == Key::Named(keyboard::key::Named::Enter) {
                        return self.update(Message::SaveClose);
                    }
                } else if let Some(message) = shortcut(&key, modifiers, self.panel) {
                    return self.update(message);
                }
            }
            Message::Quit | Message::DetachTab(_) => {}
            Message::Edit(action) => {
                let edit = action.is_edit();
                self.tabs[self.active_tab].on_edit(action);
                self.changed();
                if edit {
                    self.refresh_search();
                }
            }
            Message::Indent | Message::Unindent | Message::SmartBackspace => {
                let tab = &mut self.tabs[self.active_tab];
                match message {
                    Message::Indent => tab.indent(),
                    Message::Unindent => tab.unindent(),
                    _ => tab.smart_backspace(),
                }
                self.changed();
                self.refresh_search();
            }
            Message::SelectTab(index) => {
                if index < self.tabs.len() {
                    self.active_tab = index;
                    if self.split_anchor.is_some()
                        && !self
                            .split_pair()
                            .is_some_and(|(left, right)| index == left || index == right)
                    {
                        self.split_anchor = self.adjacent_split_anchor();
                        self.normalize_split();
                    }
                    self.context = None;
                    self.panel = Panel::None;
                    self.changed();
                    self.refresh_search();
                }
                return self.focus_editor();
            }
            Message::NextTab => {
                return self.update(Message::SelectTab((self.active_tab + 1) % self.tabs.len()));
            }
            Message::PreviousTab => {
                return self.update(Message::SelectTab(
                    (self.active_tab + self.tabs.len() - 1) % self.tabs.len(),
                ));
            }
            Message::CloseTab(index) => {
                if let Some(tab) = self.tabs.get(index) {
                    if tab.is_dirty {
                        self.close_confirmation = Some(tab.id.clone());
                        return operation::focus("close-cancel");
                    }
                    self.remove_tab(index);
                }
                return self.focus_editor();
            }
            Message::CancelClose => {
                self.close_confirmation = None;
                return self.focus_editor();
            }
            Message::DiscardClose => {
                if let Some(index) = self
                    .tabs
                    .iter()
                    .position(|t| Some(&t.id) == self.close_confirmation.as_ref())
                {
                    self.remove_tab(index);
                }
                return self.focus_editor();
            }
            Message::SaveClose => {
                if let Some(id) = self.close_confirmation.take() {
                    self.close_after_save = Some(id.clone());
                    if self
                        .tabs
                        .iter()
                        .find(|t| t.id == id)
                        .is_some_and(|t| t.path.is_some())
                    {
                        return self.save_tab(id, None);
                    }
                    return save_as_dialog(id);
                }
            }
            Message::Run(command) => {
                self.panel = Panel::None;
                self.context = None;
                match command {
                    Command::New => {
                        self.split_anchor = None;
                        let tab = self.fresh_tab();
                        self.tabs.push(tab);
                        self.active_tab = self.tabs.len() - 1;
                        self.changed();
                        self.refresh_search();
                    }
                    Command::Open => return self.update(Message::OpenFileDialog),
                    Command::Save => {
                        let tab = &self.tabs[self.active_tab];
                        if tab.path.is_some() {
                            return self.save_tab(tab.id.clone(), None);
                        }
                        return save_as_dialog(tab.id.clone());
                    }
                    Command::SaveAs => {
                        return save_as_dialog(self.tabs[self.active_tab].id.clone());
                    }
                    Command::Close => return self.update(Message::CloseTab(self.active_tab)),
                    Command::Find => return self.update(Message::ShowPanel(Panel::Find)),
                    Command::Replace => return self.update(Message::ShowPanel(Panel::Replace)),
                    Command::Goto => return self.update(Message::ShowPanel(Panel::Goto)),
                    Command::Syntax => return self.update(Message::ShowPanel(Panel::Language)),
                    Command::Undo => {
                        self.tabs[self.active_tab].undo();
                        self.changed();
                        self.refresh_search();
                    }
                    Command::Redo => {
                        self.tabs[self.active_tab].redo();
                        self.changed();
                        self.refresh_search();
                    }
                    Command::Line(action) => {
                        self.tabs[self.active_tab].line_command(action);
                        self.changed();
                        self.refresh_search();
                    }
                    Command::Split => return self.update(Message::ToggleSplit),
                    Command::Highlight => self.highlight_enabled = !self.highlight_enabled,
                    Command::Whitespace => self.show_whitespace = !self.show_whitespace,
                    Command::Utf8 => {
                        self.tabs[self.active_tab].encoding =
                            tab::FileEncoding::Utf8 { bom: false };
                        self.tabs[self.active_tab].is_dirty = true;
                        self.changed();
                        self.status_notification =
                            Some("Document converted to UTF-8; press ⌘S to save".into());
                    }
                    Command::ZoomIn => self.font_size = (self.font_size + 1.0).min(32.0),
                    Command::ZoomOut => self.font_size = (self.font_size - 1.0).max(10.0),
                    Command::ZoomReset => self.font_size = 15.0,
                }
                return self.focus_editor();
            }
            Message::OpenFileDialog => {
                return Task::perform(
                    async {
                        rfd::AsyncFileDialog::new()
                            .set_title("Open file")
                            .pick_file()
                            .await
                            .map(|f| f.path().to_owned())
                    },
                    Message::FilePicked,
                );
            }
            Message::FilePicked(path) => {
                if let Some(path) = path {
                    self.open_path(path);
                }
                return self.focus_editor();
            }
            Message::FileDropped(path) => {
                self.open_path(path);
                return self.focus_editor();
            }
            Message::SaveAsPicked(id, path) => {
                if let Some(path) = path {
                    return self.save_tab(id, Some(path));
                }
                self.close_after_save = None;
                return self.focus_editor();
            }
            Message::ShowPanel(panel) => {
                self.context = None;
                self.panel = panel;
                self.panel_query.clear();
                self.panel_index = 0;
                if panel == Panel::Language {
                    self.panel_index = highlighter::major_languages()
                        .iter()
                        .position(|(token, _)| *token == self.tabs[self.active_tab].syntax_token())
                        .unwrap_or(0);
                }
                if matches!(panel, Panel::Find | Panel::Replace) {
                    if let Some(selection) = self.tabs[self.active_tab].content.selection()
                        && !selection.contains(['\n', '\r'])
                    {
                        self.search.query = selection;
                    }
                    self.refresh_search();
                }
                return if matches!(panel, Panel::Find | Panel::Replace) {
                    self.focus_panel()
                        .chain(operation::select_all(self.panel_id()))
                } else {
                    self.focus_panel()
                };
            }
            Message::ClosePanel => {
                self.close_confirmation = None;
                self.context = None;
                self.panel = Panel::None;
                return self.focus_editor();
            }
            Message::PanelQuery(query) => {
                self.panel_query = query;
                self.panel_index = 0;
            }
            Message::PanelStep(backwards) => {
                let count = self.panel_items().len();
                if count > 0 {
                    self.panel_index = if backwards {
                        (self.panel_index + count - 1) % count
                    } else {
                        (self.panel_index + 1) % count
                    };
                }
            }
            Message::PanelSubmit => {
                if self.panel == Panel::Goto {
                    let mut parts = self.panel_query.trim().split(':');
                    let line = parts.next().and_then(|p| p.parse::<usize>().ok());
                    let col = parts
                        .next()
                        .map(|p| p.parse::<usize>().ok())
                        .unwrap_or(Some(1));
                    if let (Some(line), Some(col), None) = (line, col, parts.next())
                        && line > 0
                        && col > 0
                    {
                        self.tabs[self.active_tab].goto(line, col);
                        self.panel = Panel::None;
                        self.changed();
                        return self.focus_editor();
                    }
                    self.status_notification =
                        Some("Enter a positive line number, such as 42 or 42:8".into());
                } else if let Some(&index) = self.panel_items().get(self.panel_index) {
                    if self.panel == Panel::Palette {
                        return self.update(Message::Run(COMMANDS[index].0));
                    }
                    if self.panel == Panel::Language {
                        return self.update(Message::SelectLanguage(
                            highlighter::major_languages()[index].0.into(),
                        ));
                    }
                }
            }
            Message::SelectLanguage(token) => {
                self.tabs[self.active_tab].set_syntax(Some(token));
                self.panel = Panel::None;
                self.changed();
                return self.focus_editor();
            }
            Message::SearchQuery(query) => {
                self.search.query = query;
                self.refresh_search();
                self.find(false);
            }
            Message::Replacement(value) => self.search.replacement = value,
            Message::FindNext(backwards) => {
                self.find(backwards);
            }
            Message::ToggleCase => {
                self.search.case_sensitive = !self.search.case_sensitive;
                self.refresh_search();
                self.find(false);
            }
            Message::ToggleRegex => {
                self.search.regex = !self.search.regex;
                self.refresh_search();
                self.find(false);
            }
            Message::ReplaceOne => {
                let tab = &mut self.tabs[self.active_tab];
                if let Some(index) = self.search.current {
                    if let Some(range) = self.search.matches.get(index).cloned() {
                        let text = tab.content.text();
                        let replacement = self.search.replacement_for(&text, range.clone());
                        tab.replace_range(range.start, range.end, replacement);
                        self.changed();
                        self.refresh_search();
                        self.find(false);
                    }
                } else {
                    self.find(false);
                }
            }
            Message::ReplaceAll => {
                let tab = &mut self.tabs[self.active_tab];
                let text = tab.content.text();
                if let Some(replacement) = self.search.replace_all(&text)
                    && replacement != text
                {
                    tab.replace_range(0, text.len(), replacement);
                    self.changed();
                    self.refresh_search();
                }
            }
            Message::DismissNotification => self.status_notification = None,
        }
        Task::none()
    }

    fn open_path(&mut self, path: PathBuf) {
        let path = expand_tilde(path);
        let path = path.canonicalize().unwrap_or(path);
        if let Some(index) = self.tabs.iter().position(|t| {
            t.path
                .as_ref()
                .is_some_and(|p| p.canonicalize().unwrap_or_else(|_| p.clone()) == path)
        }) {
            let _ = self.update(Message::SelectTab(index));
            return;
        }
        match Tab::from_file(path) {
            Ok(tab) => {
                self.split_anchor = None;
                if self.tabs.len() == 1
                    && self.tabs[0].path.is_none()
                    && !self.tabs[0].is_dirty
                    && self.tabs[0].content.is_empty()
                {
                    self.tabs[0] = tab;
                    self.active_tab = 0;
                } else {
                    self.tabs.push(tab);
                    self.active_tab = self.tabs.len() - 1;
                }
                self.panel = Panel::None;
                self.changed();
                self.refresh_search();
            }
            Err(e) => self.status_notification = Some(format!("Could not open file: {e}")),
        }
    }

    fn panel_items(&self) -> Vec<usize> {
        if self.panel == Panel::Palette {
            commands::filtered(&self.panel_query)
        } else if self.panel == Panel::Language {
            let query = self.panel_query.to_lowercase();
            highlighter::major_languages()
                .iter()
                .enumerate()
                .filter(|(_, (token, name))| {
                    token.contains(&query) || name.to_lowercase().contains(&query)
                })
                .map(|(i, _)| i)
                .collect()
        } else {
            Vec::new()
        }
    }

    fn view(&self) -> Element<'_, Message> {
        let tab = &self.tabs[self.active_tab];
        let header = canvas(tab_bar::TabBar {
            tabs: &self.tabs,
            active: self.active_tab,
            split: self.split_pair().is_some(),
        })
        .width(Length::Fill)
        .height(tab_bar::HEIGHT);
        let editors: Element<_> = if let Some((left, right)) = self.split_pair() {
            row![
                self.editor_view(left, true),
                container(space::vertical())
                    .width(1)
                    .height(Length::Fill)
                    .style(|_| container::Style {
                        background: Some(theme::BORDER_COLOR.into()),
                        ..Default::default()
                    }),
                self.editor_view(right, true)
            ]
            .height(Length::Fill)
            .into()
        } else {
            self.editor_view(self.active_tab, false)
        };
        let mut body = column![header, editors];
        if let Some(message) = &self.status_notification {
            body = body.push(
                container(
                    row![
                        text(message).size(12).width(Length::Fill),
                        small_button("×", Message::DismissNotification)
                    ]
                    .spacing(8)
                    .align_y(Alignment::Center),
                )
                .padding([6, 12])
                .style(theme::header_container_style),
            );
        }
        let (line, col) = tab.cursor_position();
        let col = tab
            .content
            .line(line)
            .map(|l| l.text[..col.min(l.text.len())].chars().count())
            .unwrap_or(0);
        let path = if self.session_blocked {
            "Automatic draft recovery unavailable".into()
        } else {
            tab.path
                .as_ref()
                .map(|p| p.display().to_string())
                .unwrap_or_else(|| "Unsaved document".into())
        };
        let footer = container(
            row![
                text(path)
                    .size(12)
                    .color(theme::TEXT_MUTED)
                    .width(Length::Fill),
                small_button(&tab.language_label(), Message::Run(Command::Syntax)),
                text(tab.encoding.label()).size(12).color(theme::TEXT_MUTED),
                small_button(
                    &format!("Ln {}, Col {}", line + 1, col + 1),
                    Message::Run(Command::Goto)
                ),
            ]
            .spacing(9)
            .align_y(Alignment::Center),
        )
        .padding([4, 12])
        .style(theme::statusbar_container_style);
        #[cfg(target_os = "macos")]
        let content = column![
            container(space::vertical())
                .width(Length::Fill)
                .height(32)
                .style(theme::header_container_style),
            body,
            footer
        ];
        #[cfg(not(target_os = "macos"))]
        let content = body.push(footer);
        let base: Element<_> = container(content)
            .style(|_| container::Style {
                background: Some(theme::BG_EDITOR.into()),
                text_color: Some(theme::TEXT_PRIMARY),
                ..Default::default()
            })
            .into();
        if let Some(id) = &self.close_confirmation {
            let title = self
                .tabs
                .iter()
                .find(|t| &t.id == id)
                .map(|t| t.title.as_str())
                .unwrap_or("file");
            let dialog = column![
                text(format!("Save changes to “{title}”?")).size(17),
                text("This document has unsaved changes.")
                    .size(13)
                    .color(theme::TEXT_MUTED),
                row![
                    button("Cancel")
                        .style(theme::picker_item_style)
                        .on_press(Message::CancelClose),
                    button("Don't save")
                        .style(theme::picker_item_style)
                        .on_press(Message::DiscardClose),
                    button("Save")
                        .style(theme::primary_button_style)
                        .on_press(Message::SaveClose)
                ]
                .spacing(10)
            ]
            .spacing(18);
            return stack![
                base,
                iced::widget::opaque(
                    container(
                        container(dialog)
                            .padding(24)
                            .style(theme::picker_container_style)
                    )
                    .center(Length::Fill)
                    .style(|_| container::Style {
                        background: Some(iced::Color::from_rgba(0.0, 0.0, 0.0, 0.35).into()),
                        ..Default::default()
                    })
                )
            ]
            .into();
        }
        if self.context.is_some() || self.panel != Panel::None {
            let backdrop = mouse_area(space::horizontal().height(Length::Fill))
                .on_press(Message::ClosePanel)
                .on_right_press(Message::ClosePanel);
            let popup = responsive(move |size| {
                let (content, position): (Element<'_, Message>, iced::Point) =
                    if let Some(menu) = &self.context {
                        let height = if matches!(menu.target, context_menu::Target::Editor(_)) {
                            310.0
                        } else {
                            250.0
                        };
                        (
                            context_menu::view(menu, tab, self.tabs.len() > 1),
                            iced::Point::new(
                                menu.position.x.min((size.width - 302.0).max(6.0)),
                                menu.position.y.min((size.height - height).max(6.0)),
                            ),
                        )
                    } else {
                        let width = if matches!(self.panel, Panel::Find | Panel::Replace) {
                            690.0_f32
                        } else {
                            540.0_f32
                        }
                        .min(size.width - 24.0);
                        let content = if matches!(self.panel, Panel::Find | Panel::Replace) {
                            self.search_bar()
                        } else {
                            self.picker()
                        };
                        (
                            container(content).width(width).into(),
                            iced::Point::new((size.width - width) / 2.0, 58.0),
                        )
                    };
                pin(iced::widget::opaque(content)).position(position).into()
            });
            return stack![base, backdrop, popup].into();
        }
        base
    }

    fn editor_view(&self, index: usize, split: bool) -> Element<'_, Message> {
        let tab = &self.tabs[index];
        let editor_id = tab.id.clone();
        let editor = text_editor(&tab.content)
            .id(self.editor_id(&tab.id))
            .font(typography::MONO)
            .size(self.font_size)
            .line_height(iced::widget::text::LineHeight::Absolute(
                (self.font_size * 1.4).into(),
            ))
            .wrapping(iced::widget::text::Wrapping::None)
            .style(theme::editor_style)
            .highlight_with::<highlighter::NordHighlighter>(
                highlighter::Settings {
                    token: if self.highlight_enabled {
                        tab.syntax_token()
                    } else {
                        "txt".into()
                    },
                    show_whitespace: self.show_whitespace,
                    search: if index == self.active_tab
                        && matches!(self.panel, Panel::Find | Panel::Replace)
                    {
                        Some((
                            self.search.query.clone(),
                            self.search.case_sensitive,
                            self.search.regex,
                        ))
                    } else {
                        None
                    },
                },
                |h, _| h.to_format(),
            )
            .key_binding(move |press| {
                if !matches!(press.status, text_editor::Status::Focused { .. }) {
                    return None;
                }
                match press.key.as_ref() {
                    Key::Named(keyboard::key::Named::Tab)
                        if !press.modifiers.control() && !press.modifiers.command() =>
                    {
                        Some(text_editor::Binding::Custom(Message::ForTab(
                            editor_id.clone(),
                            Box::new(if press.modifiers.shift() {
                                Message::Unindent
                            } else {
                                Message::Indent
                            }),
                        )))
                    }
                    Key::Named(keyboard::key::Named::Backspace)
                        if !press.modifiers.alt()
                            && !press.modifiers.command()
                            && !press.modifiers.control() =>
                    {
                        Some(text_editor::Binding::Custom(Message::ForTab(
                            editor_id.clone(),
                            Box::new(Message::SmartBackspace),
                        )))
                    }
                    Key::Named(keyboard::key::Named::Escape) => Some(text_editor::Binding::Custom(
                        Message::ForTab(editor_id.clone(), Box::new(Message::ClosePanel)),
                    )),
                    _ if shortcut(&press.key, press.modifiers, Panel::None).is_some() => None,
                    _ => text_editor::Binding::from_key_press(press),
                }
            })
            .on_action(move |action| Message::EditAt(tab.id.clone(), action))
            .height(Length::Fill)
            .padding(Padding {
                top: 10.0,
                right: 16.0,
                bottom: 10.0,
                left: 16.0,
            });

        let scroll_id = tab.id.clone();
        let viewport_id = tab.id.clone();
        let scrollbar = canvas(
            scrollbar::Scrollbar::new(tab.line_count(), tab.scroll_line, move |line| {
                Message::ScrollAt(scroll_id.clone(), line)
            })
            .metrics(self.font_size * 1.4, tab.id.clone(), move |height| {
                Message::ViewportAt(viewport_id.clone(), height)
            }),
        )
        .width(14)
        .height(Length::Fill);
        let gutter = canvas(gutter::Gutter {
            top: tab.scroll_line,
            offset: tab.scroll_offset,
            total: tab.line_count(),
            cursor: tab.cursor_position().0,
            font_size: self.font_size,
        })
        .width(
            (tab.line_count().to_string().len().max(3) as f32 * self.font_size * 0.65 + 20.0)
                .max(55.0),
        )
        .height(Length::Fill);
        let mut pane = column![].height(Length::Fill).width(Length::FillPortion(1));
        if split {
            pane = pane.push(
                container(
                    row![
                        text(tab.title.clone()).size(12).width(Length::Fill),
                        text(if index == self.active_tab { "●" } else { "" })
                            .size(10)
                            .color(theme::TEXT_ACCENT)
                    ]
                    .align_y(Alignment::Center),
                )
                .padding([7, 14])
                .style(theme::header_container_style),
            );
        }
        pane = pane.push(row![gutter, editor, scrollbar].height(Length::Fill));
        mouse_area(pane)
            .on_right_press(Message::OpenEditorContext(tab.id.clone()))
            .into()
    }

    fn search_bar(&self) -> Element<'_, Message> {
        let count = if self.search.matches.is_empty() {
            "No matches".into()
        } else {
            format!(
                "{} / {}",
                self.search.current.map(|i| i + 1).unwrap_or(0),
                self.search.matches.len()
            )
        };
        let mut bar = column![
            row![
                text_input("Find…", &self.search.query)
                    .id(self.panel_id())
                    .on_input(Message::SearchQuery)
                    .on_submit(Message::FindNext(false))
                    .style(theme::text_input_style)
                    .width(Length::Fill),
                small_button(
                    if self.search.case_sensitive {
                        "Aa ✓"
                    } else {
                        "Aa"
                    },
                    Message::ToggleCase
                ),
                small_button(
                    if self.search.regex { ".* ✓" } else { ".*" },
                    Message::ToggleRegex
                ),
                text(count).size(12).color(theme::TEXT_MUTED),
                small_button("↑", Message::FindNext(true)),
                small_button("↓", Message::FindNext(false)),
                small_button("×", Message::ClosePanel),
            ]
            .spacing(8)
            .align_y(Alignment::Center)
        ]
        .spacing(6);
        if self.panel == Panel::Replace {
            bar = bar.push(
                row![
                    text_input("Replace with…", &self.search.replacement)
                        .id(self.replacement_id())
                        .on_input(Message::Replacement)
                        .on_submit(Message::ReplaceOne)
                        .style(theme::text_input_style),
                    small_button("Replace", Message::ReplaceOne),
                    small_button("Replace all", Message::ReplaceAll)
                ]
                .spacing(8)
                .align_y(Alignment::Center),
            );
        }
        if let Some(error) = &self.search.error {
            bar = bar.push(text(error).size(11).color(theme::DIRTY_DOT));
        }
        container(bar)
            .padding([8, 12])
            .style(theme::picker_container_style)
            .into()
    }

    fn picker(&self) -> Element<'_, Message> {
        let placeholder = match self.panel {
            Panel::Goto => "Go to line:column — e.g. 42:8",
            Panel::Language => "Select language…",
            _ => "Search commands…",
        };
        let mut content = column![
            text_input(placeholder, &self.panel_query)
                .id(self.panel_id())
                .on_input(Message::PanelQuery)
                .on_submit(Message::PanelSubmit)
                .padding(10)
                .style(theme::text_input_style)
        ]
        .spacing(8);
        let indices = self.panel_items();
        if self.panel != Panel::Goto {
            let mut items = column![].spacing(2);
            // Keep the keyboard selection visible without a second scroll position.
            let first = self.panel_index.saturating_sub(5);
            for (position, &index) in indices.iter().enumerate().skip(first).take(11) {
                let (label, hint, message) = if self.panel == Panel::Palette {
                    let (command, label, hint) = COMMANDS[index];
                    (label, hint, Message::Run(command))
                } else {
                    let (token, name) = highlighter::major_languages()[index];
                    (name, token, Message::SelectLanguage(token.into()))
                };
                let selected = position == self.panel_index;
                items = items.push(
                    button(
                        row![
                            text(label).size(13).width(Length::Fill),
                            text(hint).size(11).color(theme::TEXT_MUTED)
                        ]
                        .align_y(Alignment::Center),
                    )
                    .style(move |theme, status| {
                        if selected {
                            theme::tab_button_style(true)(theme, status)
                        } else {
                            theme::picker_item_style(theme, status)
                        }
                    })
                    .padding([8, 10])
                    .width(Length::Fill)
                    .on_press(message),
                );
            }
            if indices.is_empty() {
                items = items.push(text("No results").size(13).color(theme::TEXT_MUTED));
            }
            content = content.push(items).push(
                text("↑ ↓ select     ↵ run     esc close")
                    .size(11)
                    .color(theme::TEXT_MUTED),
            );
        } else {
            content = content.push(
                text("↵ go to position     esc cancel")
                    .size(11)
                    .color(theme::TEXT_MUTED),
            );
        }
        container(content)
            .padding(12)
            .width(540)
            .style(theme::picker_container_style)
            .into()
    }
}

fn small_button(label: &str, message: Message) -> Element<'static, Message> {
    button(text(label.to_owned()).size(12))
        .padding([4, 7])
        .style(theme::icon_button_style)
        .on_press(message)
        .into()
}

fn save_as_dialog(id: String) -> Task<Message> {
    Task::perform(
        async {
            rfd::AsyncFileDialog::new()
                .set_title("Save as")
                .save_file()
                .await
                .map(|f| f.path().to_owned())
        },
        move |path| Message::SaveAsPicked(id.clone(), path),
    )
}

fn shortcut(key: &Key, m: keyboard::Modifiers, panel: Panel) -> Option<Message> {
    use keyboard::key::Named;
    if *key == Key::Named(Named::Escape) {
        return Some(Message::ClosePanel);
    }
    if matches!(panel, Panel::Palette | Panel::Language) {
        if *key == Key::Named(Named::ArrowUp) {
            return Some(Message::PanelStep(true));
        }
        if *key == Key::Named(Named::ArrowDown) {
            return Some(Message::PanelStep(false));
        }
    }
    if m.command() {
        let Key::Character(c) = key else {
            return None;
        };
        let c = c.to_lowercase();
        return Some(match c.as_str() {
            "p" if m.shift() => Message::ShowPanel(Panel::Palette),
            "s" if m.shift() => Message::Run(Command::SaveAs),
            "s" => Message::Run(Command::Save),
            "o" => Message::Run(Command::Open),
            "n" | "t" => Message::Run(Command::New),
            "w" => Message::Run(Command::Close),
            "q" => Message::Quit,
            "\\" => Message::ToggleSplit,
            "f" if m.alt() => Message::Run(Command::Replace),
            "f" => Message::Run(Command::Find),
            "g" => Message::FindNext(m.shift()),
            "l" => Message::Run(Command::Syntax),
            "." => Message::Run(Command::Whitespace),
            "+" | "=" => Message::Run(Command::ZoomIn),
            "-" => Message::Run(Command::ZoomOut),
            "0" => Message::Run(Command::ZoomReset),
            "]" if m.shift() => Message::NextTab,
            "[" if m.shift() => Message::PreviousTab,
            "z" if panel == Panel::None => Message::Run(if m.shift() {
                Command::Redo
            } else {
                Command::Undo
            }),
            "d" if m.shift() && panel == Panel::None => {
                Message::Run(Command::Line(LineCommand::Duplicate))
            }
            "k" if m.shift() && panel == Panel::None => {
                Message::Run(Command::Line(LineCommand::Delete))
            }
            "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9" => {
                Message::SelectTab(c.parse::<usize>().ok()? - 1)
            }
            _ => return None,
        });
    }
    if m.control() && *key == Key::Named(Named::Tab) {
        return Some(if m.shift() {
            Message::PreviousTab
        } else {
            Message::NextTab
        });
    }
    if m.control() && key.as_ref() == Key::Character("g") {
        return Some(Message::Run(Command::Goto));
    }
    if panel == Panel::None && m.alt() {
        if *key == Key::Named(Named::ArrowUp) {
            return Some(Message::Run(Command::Line(LineCommand::MoveUp)));
        }
        if *key == Key::Named(Named::ArrowDown) {
            return Some(Message::Run(Command::Line(LineCommand::MoveDown)));
        }
    }
    None
}

fn main() -> iced::Result {
    #[cfg(target_os = "macos")]
    macos::init();
    iced::daemon(
        desktop::Desktop::new,
        desktop::Desktop::update,
        desktop::Desktop::view,
    )
    .title(desktop::Desktop::title)
    .subscription(desktop::Desktop::subscription)
    .theme(iced::Theme::Dark)
    .default_font(typography::UI)
    .font(include_bytes!("../assets/ZeroWhitespace.ttf").as_slice())
    .font(include_bytes!("../assets/fonts/JetBrainsMono-Regular.ttf").as_slice())
    .font(include_bytes!("../assets/fonts/JetBrainsMono-Bold.ttf").as_slice())
    .font(include_bytes!("../assets/fonts/JetBrainsMono-Italic.ttf").as_slice())
    .font(include_bytes!("../assets/fonts/JetBrainsMono-BoldItalic.ttf").as_slice())
    .run()
}

#[cfg(test)]
mod tests {
    use super::*;
    fn app(tabs: Vec<Tab>) -> App {
        App::from_tabs(tabs, 0, None)
    }
    #[test]
    fn closing_modified_tab_requires_a_choice() {
        let mut tab = Tab::new_empty(0);
        tab.on_edit(text_editor::Action::Edit(text_editor::Edit::Insert('x')));
        let mut app = app(vec![tab]);
        let _ = app.update(Message::CloseTab(0));
        assert!(app.close_confirmation.is_some());
        assert_eq!(app.tabs[0].content.text(), "x");
        let _ = app.update(Message::CancelClose);
        assert!(app.close_confirmation.is_none());
        assert_eq!(app.tabs[0].content.text(), "x");
    }
    #[test]
    fn closing_an_earlier_tab_keeps_active_document() {
        let mut app = app(vec![
            Tab::new_empty(0),
            Tab::new_empty(1),
            Tab::new_empty(2),
        ]);
        app.active_tab = 1;
        let id = app.tabs[1].id.clone();
        let _ = app.update(Message::CloseTab(0));
        assert_eq!(app.tabs[app.active_tab].id, id);
    }
    #[test]
    fn save_as_result_is_bound_to_its_original_tab() {
        let path = std::env::temp_dir().join(storage::unique_id());
        let mut app = app(vec![Tab::new_empty(0), Tab::new_empty(1)]);
        app.tabs[0].content = text_editor::Content::with_text("original tab");
        app.tabs[1].content = text_editor::Content::with_text("other tab");
        let id = app.tabs[0].id.clone();
        app.active_tab = 1;
        let _ = app.update(Message::SaveAsPicked(id, Some(path.clone())));
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "original tab");
        assert!(app.tabs[1].path.is_none());
        std::fs::remove_file(path).unwrap();
    }
    #[test]
    fn opening_a_file_preserves_existing_draft() {
        let path = std::env::temp_dir().join(storage::unique_id());
        std::fs::write(&path, "disk file").unwrap();
        let mut tab = Tab::new_empty(0);
        tab.content = text_editor::Content::with_text("recovered draft");
        tab.is_dirty = true;
        let mut app = app(vec![tab]);
        app.open_path(path.clone());
        assert_eq!(app.tabs.len(), 2);
        assert_eq!(app.tabs[0].content.text(), "recovered draft");
        app.open_path(path.clone());
        assert_eq!(app.tabs.len(), 2);
        std::fs::remove_file(path).unwrap();
    }
    #[test]
    fn split_edits_target_document_and_keeps_adjacent_pair() {
        let mut app = app(vec![
            Tab::new_empty(0),
            Tab::new_empty(1),
            Tab::new_empty(2),
        ]);
        let right = app.tabs[1].id.clone();
        let _ = app.update(Message::ToggleSplit);
        let _ = app.update(Message::EditAt(
            right,
            text_editor::Action::Edit(text_editor::Edit::Insert('r')),
        ));
        assert_eq!(app.split_pair(), Some((0, 1)));
        assert_eq!(app.active_tab, 1);
        assert_eq!(app.tabs[0].content.text(), "");
        assert_eq!(app.tabs[1].content.text(), "r");
        app.remove_tab(1);
        assert_eq!(app.split_pair(), Some((0, 1)));
    }
    #[test]
    fn reorder_preserves_document_identity_cursor_history_and_active_tab() {
        let mut app = app(vec![
            Tab::new_empty(0),
            Tab::new_empty(1),
            Tab::new_empty(2),
        ]);
        app.active_tab = 1;
        let id = app.tabs[1].id.clone();
        let _ = app.update(Message::Edit(text_editor::Action::Edit(
            text_editor::Edit::Insert('x'),
        )));
        let _ = app.update(Message::Tabs(tab_bar::Intent::Reorder {
            id: id.clone(),
            slot: 0,
        }));
        assert_eq!(app.active_tab, 0);
        assert_eq!(app.tabs[0].id, id);
        assert_eq!(app.tabs[0].cursor_position(), (0, 1));
        app.tabs[0].undo();
        assert_eq!(app.tabs[0].content.text(), "");
    }
    #[test]
    fn single_tab_does_not_split_or_show_a_warning() {
        let mut app = app(vec![Tab::new_empty(0)]);
        let _ = app.update(Message::ToggleSplit);
        assert!(app.split_pair().is_none());
        assert!(app.status_notification.is_none());
        let _ = app.update(Message::ShowPanel(Panel::Find));
        let _ = app.update(Message::ClosePanel);
        assert_eq!(app.panel, Panel::None);
        assert_eq!(app.tabs[0].content.text(), "");
    }
    #[test]
    fn last_tab_splits_with_previous_and_keeps_active_document() {
        let mut app = app(vec![
            Tab::new_empty(0),
            Tab::new_empty(1),
            Tab::new_empty(2),
        ]);
        app.active_tab = 2;
        let _ = app.update(Message::ToggleSplit);
        assert_eq!(app.split_pair(), Some((1, 2)));
        assert_eq!(app.active_tab, 2);
        assert!(app.status_notification.is_none());
        let _ = app.update(Message::ToggleSplit);
        assert!(app.split_pair().is_none());
        let _ = app.update(Message::SelectTab(0));
        let _ = app.update(Message::ToggleSplit);
        assert_eq!(app.split_pair(), Some((0, 1)));
        let _ = app.update(Message::SelectTab(2));
        assert_eq!(app.split_pair(), Some((1, 2)));
        assert_eq!(app.active_tab, 2);
    }
}
