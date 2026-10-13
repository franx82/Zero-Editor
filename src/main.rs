mod highlighter;
mod session;
mod tab;
mod theme;

use std::path::PathBuf;
use iced::widget::{button, column, container, row, scrollable, space, stack, text, text_editor, text_input};
use iced::{
    event, keyboard, window, Alignment, Element, Event, Font, Length, Padding,
    Size, Subscription, Task,
};
use iced::keyboard::Key;
use tab::Tab;

struct App {
    tabs: Vec<Tab>,
    active_tab: usize,
    highlight_enabled: bool,
    show_whitespace: bool,
    untitled_counter: usize,
    status_notification: Option<String>,
    language_picker_open: bool,
    language_filter: String,
}

#[derive(Debug, Clone)]
enum Message {
    Edit(text_editor::Action),
    Indent,
    Unindent,
    SmartBackspace,
    ToggleWhitespace,
    SelectTab(usize),
    CloseTab(usize),
    CloseActiveTab,
    NewTab,
    OpenFileDialog,
    FilePicked(Option<PathBuf>),
    FileDropped(PathBuf),
    Save,
    SaveAsDialog,
    SaveAsPicked(Option<PathBuf>),
    NextTab,
    PreviousTab,
    ToggleHighlight,
    OpenLanguagePicker,
    CloseLanguagePicker,
    FilterLanguages(String),
    SelectLanguage(String),
    WindowCloseRequested(window::Id),
}

impl App {
    fn new() -> (Self, Task<Message>) {
        let args: Vec<String> = std::env::args().skip(1).collect();
        let mut initial_tabs = Vec::new();

        // 1. Check if files were passed as command line arguments
        for arg in &args {
            let path = PathBuf::from(arg);
            if path.exists() {
                if let Ok(tab) = Tab::from_file(path) {
                    initial_tabs.push(tab);
                }
            }
        }

        // 2. Otherwise load persistent session
        let active_index = if initial_tabs.is_empty() {
            if let Some((saved_tabs, saved_active)) = session::load_session() {
                initial_tabs = saved_tabs;
                saved_active
            } else {
                0
            }
        } else {
            0
        };

        // 3. Fallback to an empty tab if nothing opened
        if initial_tabs.is_empty() {
            initial_tabs.push(Tab::new_empty(0));
        }

        let app = Self {
            tabs: initial_tabs,
            active_tab: active_index,
            highlight_enabled: true,
            show_whitespace: true,
            untitled_counter: 0,
            status_notification: None,
            language_picker_open: false,
            language_filter: String::new(),
        };

        let load_font = iced::font::load(include_bytes!("../assets/ZeroWhitespace.ttf")).discard();

        (app, load_font)
    }

    fn title(&self) -> String {
        if let Some(tab) = self.tabs.get(self.active_tab) {
            let dirty = if tab.is_dirty { " •" } else { "" };
            format!("{}{} — Zero Editor", tab.title, dirty)
        } else {
            "Zero Editor".to_string()
        }
    }

    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::Edit(action) => {
                if let Some(tab) = self.tabs.get_mut(self.active_tab) {
                    tab.on_edit(action);
                    // Persist session draft when changes occur
                    session::save_session(&self.tabs, self.active_tab);
                }
                Task::none()
            }

            Message::Indent => {
                if let Some(tab) = self.tabs.get_mut(self.active_tab) {
                    tab.indent();
                    session::save_session(&self.tabs, self.active_tab);
                }
                Task::none()
            }

            Message::Unindent => {
                if let Some(tab) = self.tabs.get_mut(self.active_tab) {
                    tab.unindent();
                    session::save_session(&self.tabs, self.active_tab);
                }
                Task::none()
            }

            Message::SmartBackspace => {
                if let Some(tab) = self.tabs.get_mut(self.active_tab) {
                    tab.smart_backspace();
                    session::save_session(&self.tabs, self.active_tab);
                }
                Task::none()
            }

            Message::ToggleWhitespace => {
                self.show_whitespace = !self.show_whitespace;
                Task::none()
            }

            Message::SelectTab(index) => {
                if index < self.tabs.len() {
                    self.active_tab = index;
                    session::save_session(&self.tabs, self.active_tab);
                }
                Task::none()
            }

            Message::CloseTab(index) => {
                if index < self.tabs.len() {
                    self.tabs.remove(index);
                    if self.tabs.is_empty() {
                        self.untitled_counter += 1;
                        self.tabs.push(Tab::new_empty(self.untitled_counter));
                        self.active_tab = 0;
                    } else if self.active_tab >= self.tabs.len() {
                        self.active_tab = self.tabs.len() - 1;
                    }
                    session::save_session(&self.tabs, self.active_tab);
                }
                Task::none()
            }

            Message::CloseActiveTab => {
                let idx = self.active_tab;
                self.update(Message::CloseTab(idx))
            }

            Message::NewTab => {
                self.untitled_counter += 1;
                self.tabs.push(Tab::new_empty(self.untitled_counter));
                self.active_tab = self.tabs.len() - 1;
                session::save_session(&self.tabs, self.active_tab);
                Task::none()
            }

            Message::OpenFileDialog => {
                Task::perform(
                    async {
                        let handle = rfd::AsyncFileDialog::new()
                            .set_title("Apri file")
                            .pick_file()
                            .await;
                        handle.map(|h| h.path().to_path_buf())
                    },
                    Message::FilePicked,
                )
            }

            Message::FilePicked(maybe_path) => {
                if let Some(path) = maybe_path {
                    self.open_path(path);
                }
                Task::none()
            }

            Message::FileDropped(path) => {
                self.open_path(path);
                Task::none()
            }

            Message::Save => {
                if let Some(tab) = self.tabs.get_mut(self.active_tab) {
                    if tab.path.is_some() {
                        match tab.save() {
                            Ok(()) => {
                                self.status_notification = Some("File salvato".to_string());
                                session::save_session(&self.tabs, self.active_tab);
                            }
                            Err(e) => {
                                self.status_notification = Some(format!("Errore salvataggio: {e}"));
                            }
                        }
                        Task::none()
                    } else {
                        // Untitled file requires Save As dialog
                        self.update(Message::SaveAsDialog)
                    }
                } else {
                    Task::none()
                }
            }

            Message::SaveAsDialog => {
                Task::perform(
                    async {
                        let handle = rfd::AsyncFileDialog::new()
                            .set_title("Salva con nome")
                            .save_file()
                            .await;
                        handle.map(|h| h.path().to_path_buf())
                    },
                    Message::SaveAsPicked,
                )
            }

            Message::SaveAsPicked(maybe_path) => {
                if let Some(path) = maybe_path {
                    if let Some(tab) = self.tabs.get_mut(self.active_tab) {
                        match tab.save_as(path) {
                            Ok(()) => {
                                self.status_notification = Some("File salvato".to_string());
                                session::save_session(&self.tabs, self.active_tab);
                            }
                            Err(e) => {
                                self.status_notification = Some(format!("Errore salvataggio: {e}"));
                            }
                        }
                    }
                }
                Task::none()
            }

            Message::NextTab => {
                if !self.tabs.is_empty() {
                    self.active_tab = (self.active_tab + 1) % self.tabs.len();
                    session::save_session(&self.tabs, self.active_tab);
                }
                Task::none()
            }

            Message::PreviousTab => {
                if !self.tabs.is_empty() {
                    self.active_tab = (self.active_tab + self.tabs.len() - 1) % self.tabs.len();
                    session::save_session(&self.tabs, self.active_tab);
                }
                Task::none()
            }

            Message::ToggleHighlight => {
                self.highlight_enabled = !self.highlight_enabled;
                Task::none()
            }

            Message::OpenLanguagePicker => {
                self.language_picker_open = !self.language_picker_open;
                self.language_filter.clear();
                Task::none()
            }

            Message::CloseLanguagePicker => {
                self.language_picker_open = false;
                self.language_filter.clear();
                Task::none()
            }

            Message::FilterLanguages(filter) => {
                self.language_filter = filter;
                Task::none()
            }

            Message::SelectLanguage(token) => {
                if let Some(tab) = self.tabs.get_mut(self.active_tab) {
                    tab.set_syntax(Some(token));
                    self.language_picker_open = false;
                    self.language_filter.clear();
                }
                Task::none()
            }

            Message::WindowCloseRequested(id) => {
                session::save_session(&self.tabs, self.active_tab);
                window::close(id)
            }
        }
    }

    fn open_path(&mut self, path: PathBuf) {
        // If file is already open in a tab, simply switch to it
        for (i, tab) in self.tabs.iter().enumerate() {
            if let Some(p) = &tab.path {
                if p == &path {
                    self.active_tab = i;
                    session::save_session(&self.tabs, self.active_tab);
                    return;
                }
            }
        }

        match Tab::from_file(path) {
            Ok(new_tab) => {
                // If current tab is pristine and untitled with no content, replace it
                if self.tabs.len() == 1
                    && self.tabs[0].path.is_none()
                    && !self.tabs[0].is_dirty
                    && self.tabs[0].content.is_empty()
                {
                    self.tabs[0] = new_tab;
                    self.active_tab = 0;
                } else {
                    self.tabs.push(new_tab);
                    self.active_tab = self.tabs.len() - 1;
                }
                session::save_session(&self.tabs, self.active_tab);
            }
            Err(e) => {
                self.status_notification = Some(format!("Impossibile aprire file: {e}"));
            }
        }
    }

    fn view(&self) -> Element<'_, Message> {
        let active_tab = self.tabs.get(self.active_tab);

        // Header / Tab Bar
        let tabs_list = row(
            self.tabs
                .iter()
                .enumerate()
                .map(|(i, tab)| {
                    let is_active = i == self.active_tab;

                    let mut tab_label = row![
                        text(&tab.title).size(13).font(Font::MONOSPACE),
                    ]
                    .spacing(4)
                    .align_y(Alignment::Center);

                    if tab.is_dirty {
                        tab_label = tab_label.push(text(" ●").size(10).color(theme::DIRTY_DOT));
                    }

                    container(
                        row![
                            button(tab_label)
                                .style(theme::tab_button_style(is_active))
                                .padding(Padding { top: 6.0, right: 8.0, bottom: 6.0, left: 10.0 })
                                .on_press(Message::SelectTab(i)),

                            button(text("✕").size(11))
                                .style(theme::icon_button_style)
                                .padding(Padding { top: 4.0, right: 6.0, bottom: 4.0, left: 6.0 })
                                .on_press(Message::CloseTab(i)),
                        ]
                        .spacing(2)
                        .align_y(Alignment::Center)
                    )
                    .style(move |_| container::Style {
                        background: Some(iced::Background::Color(
                            if is_active { theme::BG_TAB_ACTIVE } else { theme::BG_TAB_INACTIVE }
                        )),
                        border: iced::Border {
                            color: if is_active { theme::BORDER_ACTIVE } else { iced::Color::TRANSPARENT },
                            width: 1.0,
                            radius: iced::border::Radius {
                                top_left: 6.0,
                                top_right: 6.0,
                                bottom_left: 0.0,
                                bottom_right: 0.0,
                            },
                        },
                        ..container::Style::default()
                    })
                    .padding(Padding { top: 0.0, right: 4.0, bottom: 0.0, left: 0.0 })
                    .into()
                })
        )
        .spacing(4)
        .align_y(Alignment::Center);

        let header_actions = row![
            button(text("+").size(16))
                .style(theme::icon_button_style)
                .padding(Padding { top: 4.0, right: 8.0, bottom: 4.0, left: 8.0 })
                .on_press(Message::NewTab),

            button(text("Apri").size(12))
                .style(theme::icon_button_style)
                .padding(Padding { top: 4.0, right: 8.0, bottom: 4.0, left: 8.0 })
                .on_press(Message::OpenFileDialog),

            button(text("Salva").size(12))
                .style(theme::icon_button_style)
                .padding(Padding { top: 4.0, right: 8.0, bottom: 4.0, left: 8.0 })
                .on_press(Message::Save),
        ]
        .spacing(6)
        .align_y(Alignment::Center);

        let header = container(
            row![
                scrollable(tabs_list).direction(scrollable::Direction::Horizontal(
                    scrollable::Scrollbar::default()
                )),
                space::horizontal(),
                header_actions,
            ]
            .spacing(8)
            .align_y(Alignment::Center)
        )
        .style(theme::header_container_style)
        .padding(Padding { top: 6.0, right: 12.0, bottom: 0.0, left: 12.0 });

        // Main Editor Surface with Nord syntax highlighting
        let editor_surface: Element<'_, Message> = if let Some(tab) = active_tab {
            let syntax = if self.highlight_enabled {
                tab.syntax_token()
            } else {
                "txt".to_string()
            };

            let editor_widget = container(
                text_editor(&tab.content)
                    .font(Font::MONOSPACE)
                    .style(theme::editor_style)
                    .highlight_with::<highlighter::NordHighlighter>(
                        highlighter::Settings {
                            token: syntax,
                            show_whitespace: self.show_whitespace,
                        },
                        |highlight, _theme| highlight.to_format(),
                    )
                    .key_binding(|key_press| {
                        match key_press.key.as_ref() {
                            Key::Named(keyboard::key::Named::Tab) => {
                                if key_press.modifiers.shift() {
                                    Some(text_editor::Binding::Custom(Message::Unindent))
                                } else {
                                    Some(text_editor::Binding::Custom(Message::Indent))
                                }
                            }
                            Key::Named(keyboard::key::Named::Backspace) => {
                                if !key_press.modifiers.shift() && !key_press.modifiers.command() {
                                    Some(text_editor::Binding::Custom(Message::SmartBackspace))
                                } else {
                                    text_editor::Binding::from_key_press(key_press)
                                }
                            }
                            _ => text_editor::Binding::from_key_press(key_press),
                        }
                    })
                    .on_action(Message::Edit)
                    .height(Length::Fill)
                    .padding(Padding { top: 10.0, right: 16.0, bottom: 10.0, left: 16.0 })
            )
            .height(Length::Fill)
            .width(Length::Fill)
            .style(|_| container::Style {
                background: Some(iced::Background::Color(theme::BG_EDITOR)),
                ..container::Style::default()
            });

            if self.language_picker_open {
                let filter = self.language_filter.to_lowercase();
                let languages = highlighter::major_languages()
                    .iter()
                    .filter(|(token, name)| {
                        filter.is_empty()
                            || name.to_lowercase().contains(&filter)
                            || token.to_lowercase().contains(&filter)
                    });

                let items = column(
                    languages.map(|(token, name)| {
                        button(
                            row![
                                text(*name).size(13).font(Font::MONOSPACE),
                                space::horizontal(),
                                text(format!(".{}", token)).size(11).color(theme::TEXT_MUTED),
                            ]
                            .align_y(Alignment::Center)
                        )
                        .style(theme::picker_item_style)
                        .padding(Padding { top: 6.0, right: 10.0, bottom: 6.0, left: 10.0 })
                        .width(Length::Fill)
                        .on_press(Message::SelectLanguage(token.to_string()))
                        .into()
                    })
                )
                .spacing(2);

                let picker_modal = container(
                    column![
                        row![
                            text_input("Seleziona linguaggio (es. rust, python)...", &self.language_filter)
                                .on_input(Message::FilterLanguages)
                                .style(theme::text_input_style)
                                .padding(Padding { top: 8.0, right: 10.0, bottom: 8.0, left: 10.0 }),
                            button(text("✕").size(12))
                                .style(theme::icon_button_style)
                                .padding(Padding { top: 6.0, right: 8.0, bottom: 6.0, left: 8.0 })
                                .on_press(Message::CloseLanguagePicker)
                        ]
                        .spacing(8)
                        .align_y(Alignment::Center),
                        scrollable(items).height(Length::Fixed(240.0)),
                    ]
                    .spacing(8)
                )
                .style(theme::picker_container_style)
                .width(Length::Fixed(420.0))
                .padding(12.0);

                stack![
                    editor_widget,
                    container(picker_modal)
                        .width(Length::Fill)
                        .align_x(iced::alignment::Horizontal::Center)
                        .padding(Padding { top: 16.0, right: 0.0, bottom: 0.0, left: 0.0 })
                ]
                .into()
            } else {
                editor_widget.into()
            }
        } else {
            container(text("Nessun buffer aperto").color(theme::TEXT_MUTED))
                .center(Length::Fill)
                .into()
        };

        // Status Bar
        let (cursor_line, cursor_col) = active_tab
            .map(|t| t.cursor_position())
            .unwrap_or((0, 0));
        let total_lines = active_tab.map(|t| t.line_count()).unwrap_or(0);
        let lang_label = active_tab.map(|t| t.language_label()).unwrap_or_else(|| "Plain Text".into());
        let dirty_str = active_tab
            .map(|t| if t.is_dirty { "● Modificato" } else { "Salvato" })
            .unwrap_or("");
        let file_path_str = active_tab
            .and_then(|t| t.path.as_ref())
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|| "Senza nome (Draft)".to_string());

        let statusbar = container(
            row![
                text(format!("{}  ·  {}", file_path_str, dirty_str))
                    .size(11)
                    .color(theme::TEXT_MUTED),

                space::horizontal(),

                button(
                    row![
                        text(format!("Sintassi: {}", lang_label))
                            .size(11)
                            .color(if self.highlight_enabled { theme::TEXT_ACCENT } else { theme::TEXT_MUTED }),
                        text(if self.highlight_enabled { "✓" } else { "off" })
                            .size(10)
                            .color(if self.highlight_enabled { theme::NORD7_FROST_TEAL } else { theme::TEXT_MUTED }),
                    ]
                    .spacing(4)
                    .align_y(Alignment::Center)
                )
                .style(theme::icon_button_style)
                .padding(Padding { top: 2.0, right: 8.0, bottom: 2.0, left: 8.0 })
                .on_press(Message::OpenLanguagePicker),

                button(
                    row![
                        text(if self.highlight_enabled { "Sintassi: ON" } else { "Sintassi: OFF" })
                            .size(10)
                            .color(if self.highlight_enabled { theme::NORD8_FROST_ICE } else { theme::TEXT_MUTED })
                    ]
                    .align_y(Alignment::Center)
                )
                .style(theme::icon_button_style)
                .padding(Padding { top: 2.0, right: 6.0, bottom: 2.0, left: 6.0 })
                .on_press(Message::ToggleHighlight),

                button(
                    row![
                        text("· Dot:").size(11).color(if self.show_whitespace { theme::TEXT_ACCENT } else { theme::TEXT_MUTED }),
                        text(if self.show_whitespace { "ON" } else { "OFF" })
                            .size(10)
                            .color(if self.show_whitespace { theme::NORD7_FROST_TEAL } else { theme::TEXT_MUTED }),
                    ]
                    .spacing(4)
                    .align_y(Alignment::Center)
                )
                .style(theme::icon_button_style)
                .padding(Padding { top: 2.0, right: 6.0, bottom: 2.0, left: 6.0 })
                .on_press(Message::ToggleWhitespace),

                text(format!("Ln {}, Col {}  ·  {} righe  ·  UTF-8", cursor_line + 1, cursor_col + 1, total_lines))
                    .size(11)
                    .color(theme::TEXT_MUTED),
            ]
            .spacing(10)
            .align_y(Alignment::Center)
        )
        .style(theme::statusbar_container_style)
        .padding(Padding { top: 4.0, right: 12.0, bottom: 4.0, left: 12.0 });

        column![
            header,
            editor_surface,
            statusbar,
        ]
        .height(Length::Fill)
        .width(Length::Fill)
        .into()
    }

    fn subscription(&self) -> Subscription<Message> {
        let window_sub = window::close_requests().map(Message::WindowCloseRequested);

        let event_sub = event::listen_with(|evt, _status, _window| match evt {
            Event::Keyboard(keyboard::Event::KeyPressed {
                key,
                modifiers,
                ..
            }) => match (key.as_ref(), modifiers) {
                (Key::Character("s") | Key::Character("S"), m) if m.command() && m.shift() => {
                    Some(Message::SaveAsDialog)
                }
                (Key::Character("s") | Key::Character("S"), m) if m.command() => {
                    Some(Message::Save)
                }
                (Key::Character("o") | Key::Character("O"), m) if m.command() => {
                    Some(Message::OpenFileDialog)
                }
                (Key::Character("n") | Key::Character("N"), m) if m.command() => {
                    Some(Message::NewTab)
                }
                (Key::Character("t") | Key::Character("T"), m) if m.command() => {
                    Some(Message::NewTab)
                }
                (Key::Character("w") | Key::Character("W"), m) if m.command() => {
                    Some(Message::CloseActiveTab)
                }
                (Key::Character("d") | Key::Character("D"), m) if m.command() && m.shift() => {
                    Some(Message::ToggleWhitespace)
                }
                (Key::Character("."), m) if m.command() => {
                    Some(Message::ToggleWhitespace)
                }
                (Key::Character("l") | Key::Character("L"), m) if m.command() => {
                    Some(Message::OpenLanguagePicker)
                }
                (Key::Named(keyboard::key::Named::Escape), _) => {
                    Some(Message::CloseLanguagePicker)
                }
                (Key::Named(keyboard::key::Named::Tab), m) if m.control() && m.shift() => {
                    Some(Message::PreviousTab)
                }
                (Key::Named(keyboard::key::Named::Tab), m) if m.control() => {
                    Some(Message::NextTab)
                }
                (Key::Character("["), m) if m.command() && m.shift() => {
                    Some(Message::PreviousTab)
                }
                (Key::Character("]"), m) if m.command() && m.shift() => {
                    Some(Message::NextTab)
                }
                (Key::Character("1"), m) if m.command() => Some(Message::SelectTab(0)),
                (Key::Character("2"), m) if m.command() => Some(Message::SelectTab(1)),
                (Key::Character("3"), m) if m.command() => Some(Message::SelectTab(2)),
                (Key::Character("4"), m) if m.command() => Some(Message::SelectTab(3)),
                (Key::Character("5"), m) if m.command() => Some(Message::SelectTab(4)),
                (Key::Character("6"), m) if m.command() => Some(Message::SelectTab(5)),
                (Key::Character("7"), m) if m.command() => Some(Message::SelectTab(6)),
                (Key::Character("8"), m) if m.command() => Some(Message::SelectTab(7)),
                (Key::Character("9"), m) if m.command() => Some(Message::SelectTab(8)),
                _ => None,
            },
            Event::Window(window::Event::FileDropped(path)) => {
                Some(Message::FileDropped(path))
            }
            _ => None,
        });

        Subscription::batch(vec![window_sub, event_sub])
    }
}

fn main() -> iced::Result {
    iced::application(App::new, App::update, App::view)
        .title(App::title)
        .subscription(App::subscription)
        .window_size(Size::new(1100.0, 720.0))
        .centered()
        .exit_on_close_request(false)
        .run()
}
