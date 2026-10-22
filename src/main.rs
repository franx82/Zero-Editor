mod commands;
mod context_menu;
mod desktop;
mod drag_edge;
mod gutter;
mod highlighter;
#[cfg(target_os = "macos")]
mod macos;
mod menu;
mod prefs;
mod scrollbar;
mod search;
mod session;
mod storage;
mod tab;
mod tab_bar;
mod theme;
mod typography;
mod wheel_axis;

use commands::{COMMANDS, Command, LineCommand};
use iced::widget::{
    button, canvas, column, container, mouse_area, operation, pin, responsive, row, scrollable,
    space, stack, text, text_editor, text_input,
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
    prefs: prefs::Prefs,
    untitled_counter: usize,
    /// Persistent message for problems the user must act on.
    status_notification: Option<String>,
    /// Short confirmation shown in the status bar, cleared after `TOAST_DURATION`.
    toast: Option<(String, std::time::Instant)>,
    panel: Panel,
    panel_query: String,
    panel_index: usize,
    panel_first: usize,
    panel_scroll: f32,
    search: search::Search,
    window_width: f32,
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
    PanelScroll(iced::mouse::ScrollDelta),
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
    /// Tab id and, when dragged, the drop point relative to the window.
    DetachTab(String, Option<iced::Point>),
    DragWindow,
    CloseWindow,
    EditAt(String, text_editor::Action),
    ForTab(String, Box<Message>),
    ViewportAt(String, f32),
    HorizontalScrollAt(String, f32),
    DragOutside(String, iced::Point, iced::Size),
    ScrollAt(String, usize),
    OpenEditorContext(String),
    ContextAction(context_menu::Action),
    /// Clipboard and selection actions chosen from the menu bar.
    EditMenu(context_menu::Action),
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

fn visual_columns(text: &str) -> usize {
    text.chars().fold(0, |column, character| {
        if character == '\t' {
            column + (4 - column % 4)
        } else if character.is_control() {
            column
        } else if character.is_ascii() || character as u32 <= 0x2E7F {
            column + 1
        } else {
            column + 2
        }
    })
}

impl App {
    fn from_tabs(tabs: Vec<Tab>, active_tab: usize, status_notification: Option<String>) -> Self {
        Self {
            tabs,
            active_tab,
            prefs: prefs::Prefs::default(),
            untitled_counter: 0,
            status_notification,
            toast: None,
            panel: Panel::None,
            panel_query: String::new(),
            panel_index: 0,
            panel_first: 0,
            panel_scroll: 0.0,
            search: search::Search::default(),
            window_width: 1160.0,
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
    fn horizontal_scroll_id(&self, id: &str) -> String {
        format!("horizontal-{id}")
    }
    fn gutter_width(&self, index: usize) -> f32 {
        (self.tabs[index].line_count().to_string().len().max(3) as f32
            * self.prefs.font_size
            * 0.65
            + 20.0)
            .max(55.0)
    }
    fn editor_viewport_width(&self, index: usize, split: bool) -> f32 {
        let pane_width = if split {
            (self.window_width - 1.0).max(1.0) / 2.0
        } else {
            self.window_width
        };
        (pane_width - self.gutter_width(index) - 14.0).max(1.0)
    }
    fn content_width(&self, index: usize) -> f32 {
        let tab = &self.tabs[index];
        (0..tab.line_count())
            .filter_map(|line| tab.content.line(line))
            .map(|line| {
                visual_columns(&line.text) as f32 * self.prefs.font_size * typography::MONO_ADVANCE
            })
            .fold(0.0_f32, f32::max)
            + 32.0
    }
    fn sync_horizontal_cursor(&mut self) -> Task<Message> {
        self.reveal_horizontal_cursor(4.0)
    }
    /// Scrolls only once the cursor comes within `margin` columns of an edge;
    /// pointer actions pass zero so clicking never shifts the view.
    fn reveal_horizontal_cursor(&mut self, margin: f32) -> Task<Message> {
        if self.prefs.word_wrap {
            return Task::none();
        }

        let index = self.active_tab;
        let split = self.split_pair().is_some();
        let viewport_width = self.editor_viewport_width(index, split);
        let font_size = self.prefs.font_size;
        let scroll_id = self.horizontal_scroll_id(&self.tabs[index].id);
        let tab = &mut self.tabs[index];
        let cursor = tab.content.cursor().position;
        let line = tab
            .content
            .line(cursor.line)
            .map(|line| line.text.into_owned())
            .unwrap_or_default();
        let byte_column = cursor.column.min(line.len());
        let byte_column = (0..=byte_column)
            .rev()
            .find(|offset| line.is_char_boundary(*offset))
            .unwrap_or(0);
        let advance = font_size * typography::MONO_ADVANCE;
        let cursor_x = 16.0 + visual_columns(&line[..byte_column]) as f32 * advance;
        let margin = margin * advance;
        let current = tab.horizontal_offset;
        let target = if cursor_x < current + margin {
            let target = cursor_x - margin;
            if target < 16.0 + margin { 0.0 } else { target }
        } else if cursor_x > current + viewport_width - margin - advance {
            (cursor_x - viewport_width + margin + advance).max(0.0)
        } else {
            current
        };

        if (target - current).abs() > 0.5 {
            tab.horizontal_offset = target;
            operation::scroll_to(
                scroll_id,
                scrollable::AbsoluteOffset {
                    x: Some(target),
                    y: None,
                },
            )
        } else {
            Task::none()
        }
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

    fn notify(&mut self, message: impl Into<String>) {
        self.toast = Some((message.into(), std::time::Instant::now()));
    }

    fn expire_toast(&mut self) {
        if self
            .toast
            .as_ref()
            .is_some_and(|(_, at)| at.elapsed() >= TOAST_DURATION)
        {
            self.toast = None;
        }
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
                #[cfg(target_os = "macos")]
                if let Some(path) = &self.tabs[index].path {
                    macos::note_recent(path);
                }
                let title = self.tabs[index].title.clone();
                self.notify(format!("Saved {title}"));
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
                    Intent::Detach(id, point) => {
                        return Task::done(Message::DetachTab(id, Some(point)));
                    }
                    Intent::DragWindow => return Task::done(Message::DragWindow),
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
                return Task::batch([self.focus_editor(), self.sync_horizontal_cursor()]);
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
                        let pointer = matches!(
                            action,
                            text_editor::Action::Click(_)
                                | text_editor::Action::Drag(_)
                                | text_editor::Action::SelectWord
                                | text_editor::Action::SelectLine
                        );
                        let edit = self.update(Message::Edit(action));
                        let reveal = if pointer {
                            self.reveal_horizontal_cursor(0.0)
                        } else {
                            self.sync_horizontal_cursor()
                        };
                        return Task::batch([edit, reveal]);
                    }
                }
            }
            Message::ForTab(id, message) => {
                if let Some(index) = self.tabs.iter().position(|t| t.id == id) {
                    self.active_tab = index;
                    let task = self.update(*message);
                    return Task::batch([task, self.sync_horizontal_cursor()]);
                }
            }
            Message::ViewportAt(id, height) => {
                if let Some(tab) = self.tabs.iter_mut().find(|t| t.id == id) {
                    tab.set_viewport(height, self.prefs.font_size * 1.4);
                }
            }
            Message::ScrollAt(id, line) => {
                if let Some(tab) = self.tabs.iter_mut().find(|t| t.id == id) {
                    tab.scroll_to_line(line);
                }
            }
            Message::HorizontalScrollAt(id, offset) => {
                if let Some(tab) = self.tabs.iter_mut().find(|t| t.id == id) {
                    tab.horizontal_offset = offset;
                }
            }
            Message::DragOutside(id, pointer, size) => {
                if let Some(index) = self.tabs.iter().position(|t| t.id == id) {
                    // Scroll faster the further the pointer is past the edge.
                    let overshoot = if pointer.x < 0.0 {
                        pointer.x
                    } else {
                        pointer.x - size.width
                    };
                    let step = overshoot.signum() * (overshoot.abs() * 0.25).clamp(2.0, 40.0);
                    let max = (self.content_width(index) - size.width).max(0.0);
                    let scroll_id = self.horizontal_scroll_id(&id);
                    self.active_tab = index;
                    let tab = &mut self.tabs[index];
                    let offset = (tab.horizontal_offset + step).clamp(0.0, max);
                    tab.horizontal_offset = offset;
                    tab.on_edit(text_editor::Action::Drag(iced::Point::new(
                        pointer.x.clamp(0.0, size.width) + offset - 16.0,
                        (pointer.y.clamp(0.0, size.height) - 10.0).max(0.0),
                    )));
                    self.changed();
                    return operation::scroll_to(
                        scroll_id,
                        scrollable::AbsoluteOffset {
                            x: Some(offset),
                            y: None,
                        },
                    );
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
                            None,
                        ));
                    }
                }
                return Task::batch([self.focus_editor(), self.sync_horizontal_cursor()]);
            }
            Message::EditMenu(action) => {
                self.context = Some(context_menu::Menu {
                    target: context_menu::Target::Editor(self.tabs[self.active_tab].id.clone()),
                    position: iced::Point::ORIGIN,
                    selected: None,
                });
                return self.update(Message::ContextAction(action));
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
            Message::Quit | Message::DetachTab(..) | Message::DragWindow | Message::CloseWindow => {
            }
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
                return Task::batch([self.focus_editor(), self.sync_horizontal_cursor()]);
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
                    Command::WordWrap => {
                        self.prefs.word_wrap = !self.prefs.word_wrap;
                        self.notify(if self.prefs.word_wrap {
                            "Word wrap on"
                        } else {
                            "Word wrap off"
                        });
                    }
                    Command::Highlight => {
                        self.prefs.highlight = !self.prefs.highlight;
                        self.notify(if self.prefs.highlight {
                            "Syntax highlighting on"
                        } else {
                            "Syntax highlighting off"
                        });
                    }
                    Command::Whitespace => {
                        self.prefs.show_whitespace = !self.prefs.show_whitespace;
                        self.notify(if self.prefs.show_whitespace {
                            "Whitespace markers on"
                        } else {
                            "Whitespace markers off"
                        });
                    }
                    Command::Utf8 => {
                        self.tabs[self.active_tab].encoding =
                            tab::FileEncoding::Utf8 { bom: false };
                        self.tabs[self.active_tab].is_dirty = true;
                        self.changed();
                        self.notify("Converted to UTF-8 — save to apply");
                    }
                    Command::ZoomIn => {
                        self.prefs.font_size =
                            (self.prefs.font_size + 1.0).min(prefs::MAX_FONT_SIZE);
                        self.notify(format!("Text size {}", self.prefs.font_size));
                    }
                    Command::ZoomOut => {
                        self.prefs.font_size =
                            (self.prefs.font_size - 1.0).max(prefs::MIN_FONT_SIZE);
                        self.notify(format!("Text size {}", self.prefs.font_size));
                    }
                    Command::ZoomReset => {
                        self.prefs.font_size = prefs::DEFAULT_FONT_SIZE;
                        self.notify(format!("Text size {}", self.prefs.font_size));
                    }
                }
                return Task::batch([self.focus_editor(), self.sync_horizontal_cursor()]);
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
                let was_searching = matches!(self.panel, Panel::Find | Panel::Replace);
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
                self.panel_first = self.panel_index.saturating_sub(PICKER_ROWS / 2);
                self.panel_scroll = 0.0;
                if matches!(panel, Panel::Find | Panel::Replace) {
                    // Switching between find and replace keeps the query as typed;
                    // the selection is then just the current match.
                    if let Some(selection) = self.tabs[self.active_tab].content.selection()
                        && !was_searching
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
                self.panel_first = 0;
            }
            Message::PanelStep(backwards) => {
                let count = self.panel_items().len();
                if count > 0 {
                    self.panel_index = if backwards {
                        (self.panel_index + count - 1) % count
                    } else {
                        (self.panel_index + 1) % count
                    };
                    self.panel_first = self.panel_first.clamp(
                        self.panel_index.saturating_sub(PICKER_ROWS - 1),
                        self.panel_index,
                    );
                }
            }
            Message::PanelScroll(delta) => {
                // Trackpads report pixels: accumulate them into whole rows.
                self.panel_scroll -= match delta {
                    iced::mouse::ScrollDelta::Lines { y, .. } => y,
                    iced::mouse::ScrollDelta::Pixels { y, .. } => y / PICKER_ROW_HEIGHT,
                };
                let rows = self.panel_scroll.trunc();
                self.panel_scroll -= rows;
                let last = self.panel_items().len().saturating_sub(PICKER_ROWS);
                self.panel_first =
                    (self.panel_first as f32 + rows).clamp(0.0, last as f32) as usize;
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
                        return Task::batch([self.focus_editor(), self.sync_horizontal_cursor()]);
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
                return self.sync_horizontal_cursor();
            }
            Message::Replacement(value) => self.search.replacement = value,
            Message::FindNext(backwards) => {
                self.find(backwards);
                return self.sync_horizontal_cursor();
            }
            Message::ToggleCase => {
                self.search.case_sensitive = !self.search.case_sensitive;
                self.refresh_search();
                self.find(false);
                return self.sync_horizontal_cursor();
            }
            Message::ToggleRegex => {
                self.search.regex = !self.search.regex;
                self.refresh_search();
                self.find(false);
                return self.sync_horizontal_cursor();
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
                return self.sync_horizontal_cursor();
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
                return self.sync_horizontal_cursor();
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
                #[cfg(target_os = "macos")]
                if let Some(path) = &tab.path {
                    macos::note_recent(path);
                }
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

    /// On/off state shown next to toggle commands in the palette.
    fn command_state(&self, command: Command) -> Option<bool> {
        match command {
            Command::WordWrap => Some(self.prefs.word_wrap),
            Command::Whitespace => Some(self.prefs.show_whitespace),
            Command::Highlight => Some(self.prefs.highlight),
            Command::Split => Some(self.split_pair().is_some()),
            _ => None,
        }
    }

    /// Message for a menu-bar choice made with the mouse.
    fn menu_message(&self, action: menu::Action) -> Message {
        use menu::Action;
        match action {
            Action::Run(command) => Message::Run(command),
            Action::Palette => Message::ShowPanel(Panel::Palette),
            Action::Copy => Message::EditMenu(context_menu::Action::Copy),
            Action::Cut => Message::EditMenu(context_menu::Action::Cut),
            Action::Paste => Message::EditMenu(context_menu::Action::Paste),
            Action::SelectAll => Message::EditMenu(context_menu::Action::SelectAll),
            Action::FindNext(backwards) => Message::FindNext(backwards),
            Action::NextTab => Message::NextTab,
            Action::PreviousTab => Message::PreviousTab,
            Action::MoveToWindow => Message::DetachTab(self.tabs[self.active_tab].id.clone(), None),
            Action::CloseWindow => Message::CloseWindow,
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
                    .style(theme::hairline_style),
                self.editor_view(right, true)
            ]
            .height(Length::Fill)
            .into()
        } else {
            self.editor_view(self.active_tab, false)
        };
        let mut body = column![header, editors];
        if let Some(message) = &self.status_notification {
            body = body.push(hairline()).push(
                container(
                    row![
                        text("●").size(8).color(theme::TEXT_ACCENT),
                        text(message)
                            .size(theme::TEXT_SM)
                            .color(theme::TEXT_BODY)
                            .width(Length::Fill),
                        small_button("×", Message::DismissNotification)
                    ]
                    .spacing(10)
                    .align_y(Alignment::Center),
                )
                .padding([5, 12])
                .style(theme::chrome_container_style),
            );
        }
        let (line, col) = tab.cursor_position();
        let col = tab
            .content
            .line(line)
            .map(|l| l.text[..col.min(l.text.len())].chars().count())
            .unwrap_or(0);
        // Folder in faint text, file name brighter: the eye lands on what is open.
        let location: Element<_> = if let Some((message, _)) = &self.toast {
            row![
                text("●").size(7).color(theme::TEXT_ACCENT),
                text(message.clone())
                    .size(theme::TEXT_SM)
                    .color(theme::TEXT_BODY)
            ]
            .spacing(7)
            .align_y(Alignment::Center)
            .into()
        } else if self.session_blocked {
            text("Automatic draft recovery unavailable")
                .size(theme::TEXT_SM)
                .color(theme::DIRTY_DOT)
                .into()
        } else if let Some(path) = &tab.path {
            let folder = path
                .parent()
                .map(|p| abbreviate_home(p) + "/")
                .unwrap_or_default();
            let name = path
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default();
            row![
                text(folder).size(theme::TEXT_SM).color(theme::TEXT_FAINT),
                text(name).size(theme::TEXT_SM).color(theme::TEXT_SECONDARY)
            ]
            .into()
        } else {
            text("Unsaved document")
                .size(theme::TEXT_SM)
                .color(theme::TEXT_FAINT)
                .into()
        };
        let footer = container(
            row![
                container(location).width(Length::Fill).clip(true),
                small_button(&tab.language_label(), Message::Run(Command::Syntax)),
                text(tab.encoding.label())
                    .size(theme::TEXT_SM)
                    .color(theme::TEXT_MUTED),
                small_button(
                    &match tab.content.selection().map(|s| s.chars().count()) {
                        Some(count) => {
                            format!("Ln {}, Col {} · {count} selected", line + 1, col + 1)
                        }
                        None => format!("Ln {}, Col {}", line + 1, col + 1),
                    },
                    Message::Run(Command::Goto)
                ),
            ]
            .spacing(6)
            .align_y(Alignment::Center),
        )
        .padding([2, 8])
        .style(theme::chrome_container_style);
        let content = body.push(hairline()).push(footer);
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
                text(format!("Save changes to “{title}”?"))
                    .size(theme::TEXT_TITLE)
                    .color(theme::TEXT_PRIMARY),
                text("Your changes will be lost if you don't save them.")
                    .size(theme::TEXT_MD)
                    .color(theme::TEXT_MUTED),
                row![
                    button(text("Don't save").size(theme::TEXT_MD))
                        .padding([6, 12])
                        .style(theme::picker_item_style)
                        .on_press(Message::DiscardClose),
                    space::horizontal(),
                    button(text("Cancel").size(theme::TEXT_MD))
                        .padding([6, 12])
                        .style(theme::picker_item_style)
                        .on_press(Message::CancelClose),
                    button(text("Save").size(theme::TEXT_MD))
                        .padding([6, 16])
                        .style(theme::primary_button_style)
                        .on_press(Message::SaveClose)
                ]
                .spacing(8)
                .padding(iced::Padding::ZERO.top(8))
            ]
            .width(400)
            .spacing(8);
            return stack![
                base,
                iced::widget::opaque(
                    container(
                        container(dialog)
                            .padding([20, 22])
                            .style(theme::picker_container_style)
                    )
                    .center(Length::Fill)
                    .style(|_| container::Style {
                        background: Some(iced::Color::from_rgba(0.0, 0.0, 0.0, 0.3).into()),
                        ..Default::default()
                    })
                )
            ]
            .into();
        }
        // Find stays docked and non-modal: the document remains clickable and editable.
        if self.context.is_none() && matches!(self.panel, Panel::Find | Panel::Replace) {
            let bar = responsive(move |size| {
                let width = 520.0_f32.min(size.width - 24.0);
                pin(iced::widget::opaque(
                    container(self.search_bar()).width(width),
                ))
                .position(iced::Point::new(
                    size.width - width - 20.0,
                    tab_bar::HEIGHT + 8.0,
                ))
                .into()
            });
            return stack![base, bar].into();
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
                            iced::Point::new((size.width - width) / 2.0, tab_bar::HEIGHT + 10.0),
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
        let viewport_width = self.editor_viewport_width(index, split);
        let editor_width = self.content_width(index).max(viewport_width);
        let mut editor = text_editor(&tab.content)
            .id(self.editor_id(&tab.id))
            .font(typography::MONO)
            .size(self.prefs.font_size)
            .line_height(iced::widget::text::LineHeight::Absolute(
                (self.prefs.font_size * 1.4).into(),
            ))
            .wrapping(if self.prefs.word_wrap {
                iced::widget::text::Wrapping::WordOrGlyph
            } else {
                iced::widget::text::Wrapping::None
            })
            .style(theme::editor_style)
            .highlight_with::<highlighter::NordHighlighter>(
                highlighter::Settings {
                    token: if self.prefs.highlight {
                        tab.syntax_token()
                    } else {
                        "txt".into()
                    },
                    show_whitespace: self.prefs.show_whitespace,
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
        if !self.prefs.word_wrap {
            editor = editor.width(iced::Pixels(editor_width));
        }

        let scroll_id = tab.id.clone();
        let viewport_id = tab.id.clone();
        let scrollbar = canvas(
            scrollbar::Scrollbar::new(tab.line_count(), tab.scroll_line, move |line| {
                Message::ScrollAt(scroll_id.clone(), line)
            })
            .metrics(self.prefs.font_size * 1.4, tab.id.clone(), move |height| {
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
            font_size: self.prefs.font_size,
        })
        .width(self.gutter_width(index))
        .height(Length::Fill);
        let mut pane = column![].height(Length::Fill).width(Length::FillPortion(1));
        if split {
            let focused = index == self.active_tab;
            pane = pane
                .push(
                    container(
                        row![
                            text(if focused { "●" } else { "" })
                                .size(8)
                                .color(theme::TEXT_ACCENT),
                            text(tab.title.clone())
                                .size(theme::TEXT_SM)
                                .color(if focused {
                                    theme::TEXT_PRIMARY
                                } else {
                                    theme::TEXT_MUTED
                                })
                                .width(Length::Fill),
                        ]
                        .spacing(8)
                        .align_y(Alignment::Center),
                    )
                    .padding([5, 14]),
                )
                .push(hairline());
        }
        if self.prefs.word_wrap {
            pane = pane.push(row![editor].height(Length::Fill));
        } else {
            let scroll_tab_id = tab.id.clone();
            let drag_tab_id = tab.id.clone();
            let horizontal_editor = scrollable(wheel_axis::wheel_axis(editor))
                .id(self.horizontal_scroll_id(&tab.id))
                .direction(scrollable::Direction::Horizontal(
                    scrollable::Scrollbar::new().width(8).scroller_width(8),
                ))
                .width(Length::Fill)
                .height(Length::Fill)
                .on_scroll(move |viewport| {
                    Message::HorizontalScrollAt(scroll_tab_id.clone(), viewport.absolute_offset().x)
                });
            let horizontal_editor =
                drag_edge::drag_edge(horizontal_editor, move |pointer, size| {
                    Message::DragOutside(drag_tab_id.clone(), pointer, size)
                });
            let band = canvas(gutter::LineBand {
                top: tab.scroll_line,
                offset: tab.scroll_offset,
                cursor: tab.cursor_position().0,
                font_size: self.prefs.font_size,
            })
            .width(Length::Fill)
            .height(Length::Fill);
            pane = pane.push(
                row![gutter, stack![band, horizontal_editor], scrollbar].height(Length::Fill),
            );
        }
        mouse_area(pane)
            .on_right_press(Message::OpenEditorContext(tab.id.clone()))
            .into()
    }

    fn search_bar(&self) -> Element<'_, Message> {
        let replacing = self.panel == Panel::Replace;
        let (count, count_color) = if self.search.query.is_empty() {
            (String::new(), theme::TEXT_MUTED)
        } else if self.search.matches.is_empty() {
            ("No results".into(), theme::DIRTY_DOT)
        } else {
            (
                format!(
                    "{} of {}",
                    self.search.current.map(|i| i + 1).unwrap_or(0),
                    self.search.matches.len()
                ),
                theme::TEXT_MUTED,
            )
        };
        let find = row![
            text_input("Find", &self.search.query)
                .id(self.panel_id())
                .padding([5, 9])
                .size(theme::TEXT_MD)
                .on_input(Message::SearchQuery)
                .on_submit(Message::FindNext(false))
                .style(theme::text_input_style)
                .width(Length::Fill),
            tip(
                toggle_button("Aa", self.search.case_sensitive, Message::ToggleCase),
                "Match case"
            ),
            tip(
                toggle_button(".*", self.search.regex, Message::ToggleRegex),
                "Use regular expression"
            ),
            text(count)
                .size(theme::TEXT_SM)
                .color(count_color)
                .width(72)
                .align_x(iced::alignment::Horizontal::Center),
            tip(
                small_button("↑", Message::FindNext(true)),
                "Previous match  ⌘⇧G"
            ),
            tip(
                small_button("↓", Message::FindNext(false)),
                "Next match  ⌘G"
            ),
            tip(small_button("×", Message::ClosePanel), "Close  esc"),
        ]
        .spacing(2)
        .align_y(Alignment::Center);
        let mut fields = column![find].spacing(6);
        if replacing {
            fields = fields.push(
                row![
                    text_input("Replace", &self.search.replacement)
                        .id(self.replacement_id())
                        .padding([5, 9])
                        .size(theme::TEXT_MD)
                        .on_input(Message::Replacement)
                        .on_submit(Message::ReplaceOne)
                        .style(theme::text_input_style)
                        .width(Length::Fill),
                    tip(small_button("Replace", Message::ReplaceOne), "Replace  ↵"),
                    small_button("All", Message::ReplaceAll),
                ]
                .spacing(2)
                .align_y(Alignment::Center),
            );
        }
        if let Some(error) = &self.search.error {
            fields = fields.push(text(error).size(theme::TEXT_XS).color(theme::DIRTY_DOT));
        }
        let expand = tip(
            button(
                text(if replacing { "⌄" } else { "›" })
                    .size(theme::TEXT_MD)
                    .align_x(iced::alignment::Horizontal::Center)
                    .width(Length::Fill),
            )
            .width(20)
            .padding([3, 0])
            .style(theme::icon_button_style)
            .on_press(Message::ShowPanel(if replacing {
                Panel::Find
            } else {
                Panel::Replace
            })),
            if replacing {
                "Hide replace"
            } else {
                "Show replace  ⌘⌥F"
            },
        );
        container(row![expand, fields].spacing(4).align_y(Alignment::Start))
            .padding(6)
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
                .padding([8, 10])
                .size(theme::TEXT_MD)
                .style(theme::text_input_style)
        ]
        .spacing(8);
        let indices = self.panel_items();
        if self.panel != Panel::Goto {
            let mut items = column![].spacing(2);
            for (position, &index) in indices
                .iter()
                .enumerate()
                .skip(self.panel_first)
                .take(PICKER_ROWS)
            {
                let (label, hint, message, state) = if self.panel == Panel::Palette {
                    let (command, label, hint) = COMMANDS[index];
                    (
                        label,
                        hint,
                        Message::Run(command),
                        self.command_state(command),
                    )
                } else {
                    let (token, name) = highlighter::major_languages()[index];
                    let current = token == self.tabs[self.active_tab].syntax_token();
                    (
                        name,
                        token,
                        Message::SelectLanguage(token.into()),
                        current.then_some(true),
                    )
                };
                let selected = position == self.panel_index;
                let state = match state {
                    Some(true) => text("on").size(theme::TEXT_XS).color(theme::TEXT_ACCENT),
                    Some(false) => text("off").size(theme::TEXT_XS).color(theme::TEXT_FAINT),
                    None => text(""),
                };
                items = items.push(
                    button(
                        row![
                            text(label).size(theme::TEXT_MD).width(Length::Fill),
                            state,
                            text(hint)
                                .size(theme::TEXT_XS)
                                .color(theme::TEXT_MUTED)
                                .width(48)
                                .align_x(iced::alignment::Horizontal::Right)
                        ]
                        .spacing(10)
                        .align_y(Alignment::Center),
                    )
                    .style(if selected {
                        theme::selected_item_style
                    } else {
                        theme::picker_item_style
                    })
                    .padding([8, 10])
                    .width(Length::Fill)
                    .on_press(message),
                );
            }
            if indices.is_empty() {
                items = items.push(
                    container(
                        text("No results")
                            .size(theme::TEXT_MD)
                            .color(theme::TEXT_MUTED),
                    )
                    .padding([8, 10]),
                );
            }
            content = content
                .push(mouse_area(items).on_scroll(Message::PanelScroll))
                .push(
                    container(
                        text("↑↓ select    ↵ run    esc close")
                            .size(theme::TEXT_XS)
                            .color(theme::TEXT_FAINT),
                    )
                    .padding([0, 10]),
                );
        } else {
            content = content.push(
                container(
                    text("↵ go to position    esc cancel")
                        .size(theme::TEXT_XS)
                        .color(theme::TEXT_FAINT),
                )
                .padding([0, 10]),
            );
        }
        container(content)
            .padding(8)
            .width(540)
            .style(theme::picker_container_style)
            .into()
    }
}

const TOAST_DURATION: std::time::Duration = std::time::Duration::from_millis(2200);

/// Rows the command and language pickers show at once.
const PICKER_ROWS: usize = 11;
/// Height of one picker row: 13px label plus 8px vertical padding and 2px spacing.
const PICKER_ROW_HEIGHT: f32 = 34.0;

fn small_button(label: &str, message: Message) -> Element<'static, Message> {
    button(text(label.to_owned()).size(theme::TEXT_SM))
        .padding([3, 8])
        .style(theme::icon_button_style)
        .on_press(message)
        .into()
}

fn toggle_button(label: &str, on: bool, message: Message) -> Element<'static, Message> {
    button(text(label.to_owned()).size(theme::TEXT_SM))
        .padding([3, 8])
        .style(theme::toggle_button_style(on))
        .on_press(message)
        .into()
}

/// Hover label for an icon-only control.
fn tip<'a>(content: impl Into<Element<'a, Message>>, label: &str) -> Element<'a, Message> {
    iced::widget::tooltip(
        content,
        container(
            text(label.to_owned())
                .size(theme::TEXT_XS)
                .color(theme::TEXT_BODY),
        )
        .padding([4, 8])
        .style(theme::tooltip_style),
        iced::widget::tooltip::Position::Bottom,
    )
    .gap(6)
    .delay(std::time::Duration::from_millis(450))
    .into()
}

fn hairline() -> Element<'static, Message> {
    container(space::horizontal())
        .width(Length::Fill)
        .height(1)
        .style(theme::hairline_style)
        .into()
}

/// `/Users/me/dev` → `~/dev`.
fn abbreviate_home(path: &std::path::Path) -> String {
    match dirs::home_dir().and_then(|home| path.strip_prefix(home).ok().map(PathBuf::from)) {
        Some(rest) if rest.as_os_str().is_empty() => "~".into(),
        Some(rest) => format!("~/{}", rest.display()),
        None => path.display().to_string(),
    }
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
    // Pickers own the keyboard; the find bar leaves editing shortcuts active.
    let editing = !matches!(panel, Panel::Palette | Panel::Goto | Panel::Language);
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
            "w" if m.alt() => Message::Run(Command::WordWrap),
            "w" if m.shift() => Message::CloseWindow,
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
            "z" if editing => Message::Run(if m.shift() {
                Command::Redo
            } else {
                Command::Undo
            }),
            "d" if m.shift() && editing => Message::Run(Command::Line(LineCommand::Duplicate)),
            "k" if m.shift() && editing => Message::Run(Command::Line(LineCommand::Delete)),
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
    if editing && m.alt() {
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
    .font(include_bytes!("../assets/fonts/MesloLGM-Regular.ttf").as_slice())
    .font(include_bytes!("../assets/fonts/MesloLGM-Bold.ttf").as_slice())
    .font(include_bytes!("../assets/fonts/MesloLGM-Italic.ttf").as_slice())
    .font(include_bytes!("../assets/fonts/MesloLGM-BoldItalic.ttf").as_slice())
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
