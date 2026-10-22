//! One session writer owns the documents across every native window.
use crate::{App, Message, Tab, session};
use iced::{Element, Event, Point, Rectangle, Size, Subscription, Task, event, mouse, window};
use std::{
    collections::BTreeMap,
    fs::File,
    time::{Duration, Instant},
};

#[derive(Debug, Clone)]
pub enum EventMessage {
    Window(window::Id, Message),
    Opened(window::Id),
    Close(window::Id),
    Event(Event, event::Status, window::Id),
    Tick,
    Saved(u64, Result<(), String>),
    Placed(window::Id, Option<Point>, Size),
}

pub struct Desktop {
    windows: BTreeMap<window::Id, App>,
    /// Screen frames (logical points) for routing tabs dropped on another window.
    frames: BTreeMap<window::Id, Rectangle>,
    parked: Vec<Tab>,
    prefs: crate::prefs::Prefs,
    focused: window::Id,
    revision: u64,
    persisted: u64,
    saving: bool,
    blocked: bool,
    last_change: Instant,
    closing: Option<window::Id>,
    quitting: bool,
    _lock: Option<File>,
}

impl Desktop {
    pub fn new() -> (Self, Task<EventMessage>) {
        let lock = session::lock_session();
        let mut blocked = lock.is_err();
        let mut notification = lock
            .as_ref()
            .err()
            .map(|e| format!("Automatic drafts unavailable: {e}. Save your documents manually."));
        let restored = if !blocked {
            match session::load_session() {
                Ok(value) => value,
                Err(e) => {
                    blocked = true;
                    notification = Some(format!(
                        "Could not restore session: {e}. The original data has been preserved."
                    ));
                    None
                }
            }
        } else {
            None
        };
        let (tabs, active) = restored.unwrap_or_else(|| (vec![Tab::new_empty(0)], 0));
        let prefs = crate::prefs::Prefs::load();
        let mut app = App::from_tabs(tabs, active, notification);
        app.prefs = prefs;
        for path in std::env::args_os().skip(1) {
            app.open_path(path.into());
        }
        #[cfg(target_os = "macos")]
        for path in crate::macos::take_initial_files() {
            app.open_path(path);
        }
        app.session_blocked = blocked;
        let revision = app.revision;
        let (id, open) = window::open(settings());
        (
            Self {
                windows: BTreeMap::from([(id, app)]),
                frames: BTreeMap::new(),
                parked: Vec::new(),
                prefs,
                focused: id,
                revision,
                persisted: 0,
                saving: false,
                blocked,
                last_change: Instant::now(),
                closing: None,
                quitting: false,
                _lock: lock.ok(),
            },
            open.map(EventMessage::Opened),
        )
    }

    pub fn title(&self, id: window::Id) -> String {
        self.windows
            .get(&id)
            .map(App::title)
            .unwrap_or_else(|| "Zero Editor".into())
    }
    pub fn view(&self, id: window::Id) -> Element<'_, EventMessage> {
        if let Some(app) = self.windows.get(&id) {
            app.view().map(move |m| EventMessage::Window(id, m))
        } else {
            iced::widget::text("Closing…").into()
        }
    }
    fn changed(&mut self) {
        self.revision += 1;
        self.last_change = Instant::now();
    }
    fn snapshot(&self) -> session::Snapshot {
        let mut tabs: Vec<&Tab> = self.parked.iter().collect();
        let mut active = 0;
        for (id, app) in &self.windows {
            if *id == self.focused {
                active = tabs.len() + app.active_tab;
            }
            tabs.extend(app.tabs.iter());
        }
        session::Snapshot::capture_iter(tabs, active)
    }
    fn persist(&mut self) -> Task<EventMessage> {
        if self.saving || self.blocked || self.revision == self.persisted {
            return Task::none();
        }
        self.saving = true;
        let snapshot = self.snapshot();
        let revision = self.revision;
        let (sender, receiver) = iced::futures::channel::oneshot::channel();
        std::thread::spawn(move || {
            let _ = sender.send(snapshot.save().map_err(|e| e.to_string()));
        });
        Task::perform(
            async move {
                receiver
                    .await
                    .unwrap_or_else(|_| Err("Background save interrupted".into()))
            },
            move |result| EventMessage::Saved(revision, result),
        )
    }
    fn finish_close(&mut self, id: window::Id) -> Task<EventMessage> {
        self.closing = None;
        self.frames.remove(&id);
        if let Some(app) = self.windows.remove(&id) {
            self.parked.extend(
                app.tabs
                    .into_iter()
                    .filter(|t| t.path.is_some() || !t.content.is_empty()),
            );
        }
        if self.windows.is_empty() {
            return window::close(id).chain(iced::exit());
        }
        if self.focused == id {
            self.focused = *self.windows.keys().next().expect("remaining window");
        }
        self.changed();
        window::close(id)
    }
    fn request_close(&mut self, id: window::Id) -> Task<EventMessage> {
        if self.blocked
            && self
                .windows
                .get(&id)
                .is_some_and(|a| a.tabs.iter().any(|t| t.is_dirty))
        {
            if let Some(app) = self.windows.get_mut(&id) {
                app.status_notification=Some("Save your modified documents before quitting: automatic drafts are unavailable.".into());
            }
            return Task::none();
        }
        if !self.saving && (self.blocked || self.revision == self.persisted) {
            return self.finish_close(id);
        }
        self.closing = Some(id);
        self.persist()
    }
    fn quit(&mut self) -> Task<EventMessage> {
        if self.blocked
            && self
                .windows
                .values()
                .any(|a| a.tabs.iter().any(|t| t.is_dirty))
        {
            if let Some(app) = self.windows.get_mut(&self.focused) {
                app.status_notification =
                    Some("Save all modified documents before quitting.".into());
            }
            return Task::none();
        }
        if !self.saving && (self.blocked || self.revision == self.persisted) {
            return iced::exit();
        }
        self.quitting = true;
        self.persist()
    }
    /// Moves a tab out of `source`: into the window under `drop` if there is
    /// one, otherwise into a new window opened at the drop point.
    fn detach(
        &mut self,
        source: window::Id,
        tab_id: String,
        drop: Option<Point>,
    ) -> Task<EventMessage> {
        let screen = drop.and_then(|p| {
            let frame = self.frames.get(&source)?;
            Some(Point::new(frame.x + p.x, frame.y + p.y))
        });
        let target = screen.and_then(|point| {
            self.frames
                .iter()
                .find(|(id, frame)| {
                    **id != source && self.windows.contains_key(id) && frame.contains(point)
                })
                .map(|(id, frame)| (*id, Point::new(point.x - frame.x, point.y - frame.y)))
        });
        let Some(app) = self.windows.get_mut(&source) else {
            return Task::none();
        };
        let Some(index) = app.tabs.iter().position(|t| t.id == tab_id) else {
            return Task::none();
        };
        // A window's only tab has nowhere new to go.
        if target.is_none() && app.tabs.len() == 1 {
            return Task::none();
        }
        let active = app.tabs[app.active_tab].id.clone();
        let tab = app.tabs.remove(index);
        let emptied = app.tabs.is_empty();
        if !emptied {
            app.active_tab = app
                .tabs
                .iter()
                .position(|t| t.id == active)
                .unwrap_or_else(|| index.min(app.tabs.len() - 1));
            app.normalize_split();
            app.context = None;
            app.panel = crate::Panel::None;
            app.refresh_search();
        }
        self.changed();

        if let Some((target, local)) = target {
            let close = if emptied {
                self.windows.remove(&source);
                self.frames.remove(&source);
                window::close(source)
            } else {
                Task::none()
            };
            let Some(dest) = self.windows.get_mut(&target) else {
                return close;
            };
            let at = if local.y <= crate::tab_bar::HEIGHT {
                crate::tab_bar::drop_index(&dest.tabs, local.x)
            } else {
                dest.tabs.len()
            };
            dest.tabs.insert(at, tab);
            dest.active_tab = at;
            if dest.split_anchor.is_some() {
                dest.split_anchor = dest.adjacent_split_anchor();
                dest.normalize_split();
            }
            dest.refresh_search();
            self.focused = target;
            let focus = dest
                .focus_editor()
                .map(move |m| EventMessage::Window(target, m));
            return Task::batch([close, window::gain_focus(target), focus]);
        }

        let mut new = App::from_tabs(vec![tab], 0, None);
        new.prefs = self.prefs;
        new.session_blocked = self.blocked;
        let mut settings = settings();
        if let Some(point) = screen {
            // Open with the tab's title under the pointer.
            settings.position = window::Position::Specific(Point::new(
                point.x - 140.0,
                point.y - crate::tab_bar::HEIGHT / 2.0,
            ));
        }
        let (id, open) = window::open(settings);
        self.windows.insert(id, new);
        open.map(EventMessage::Opened)
    }
    fn dispatch(&mut self, id: window::Id, message: Message) -> Task<EventMessage> {
        if !self.windows.contains_key(&id) {
            return Task::none();
        }

        if let Message::FilePicked(Some(path)) | Message::FileDropped(path) = &message {
            let path = crate::expand_tilde(path.clone());
            let path = path.canonicalize().unwrap_or(path);
            let existing = self.windows.iter().find_map(|(owner, app)| {
                app.tabs
                    .iter()
                    .position(|t| {
                        t.path
                            .as_ref()
                            .is_some_and(|p| p.canonicalize().unwrap_or_else(|_| p.clone()) == path)
                    })
                    .map(|index| (*owner, index))
            });
            if let Some((owner, index)) = existing {
                self.focused = owner;
                let task = self.dispatch(owner, Message::SelectTab(index));
                return Task::batch([window::gain_focus(owner), task]);
            }
            if let Some(index) = self.parked.iter().position(|t| {
                t.path
                    .as_ref()
                    .is_some_and(|p| p.canonicalize().unwrap_or_else(|_| p.clone()) == path)
            }) {
                let tab = self.parked.remove(index);
                if let Some(app) = self.windows.get_mut(&id) {
                    app.tabs.push(tab);
                    app.active_tab = app.tabs.len() - 1;
                    app.split_anchor = None;
                    app.refresh_search();
                }
                self.changed();
                return self
                    .windows
                    .get(&id)
                    .map(|app| app.focus_editor().map(move |m| EventMessage::Window(id, m)))
                    .unwrap_or_else(Task::none);
            }
        }
        match message {
            Message::Quit => return self.quit(),
            Message::DetachTab(tab_id, drop) => return self.detach(id, tab_id, drop),
            Message::DragWindow => return window::drag(id),
            Message::CloseWindow => return self.request_close(id),
            _ => {}
        }
        let Some(app) = self.windows.get_mut(&id) else {
            return Task::none();
        };
        let before = app.revision;
        let task = app.update(message);
        let (edited, prefs) = (app.revision != before, app.prefs);
        if edited {
            self.changed();
        }
        // View preferences are global: a change in one window applies to all.
        if prefs != self.prefs {
            self.prefs = prefs;
            self.prefs.save();
            #[cfg(target_os = "macos")]
            crate::macos::set_menu_checks(&self.prefs);
            for other in self.windows.values_mut() {
                other.prefs = self.prefs;
            }
        }
        task.map(move |m| EventMessage::Window(id, m))
    }
    pub fn update(&mut self, message: EventMessage) -> Task<EventMessage> {
        match message {
            EventMessage::Window(id, message) => self.dispatch(id, message),
            EventMessage::Opened(id) => {
                self.focused = id;
                #[cfg(target_os = "macos")]
                {
                    crate::macos::style_titlebar();
                    crate::macos::set_menu_checks(&self.prefs);
                    crate::macos::install_menu();
                }
                self.windows
                    .get(&id)
                    .map(|app| {
                        Task::batch([
                            window::gain_focus(id),
                            app.focus_editor().map(move |m| EventMessage::Window(id, m)),
                            window::size(id).then(move |size| {
                                window::position(id)
                                    .map(move |position| EventMessage::Placed(id, position, size))
                            }),
                        ])
                    })
                    .unwrap_or_else(Task::none)
            }
            EventMessage::Placed(id, position, size) => {
                if let Some(origin) = position
                    && self.windows.contains_key(&id)
                {
                    self.frames.insert(id, Rectangle::new(origin, size));
                }
                Task::none()
            }
            EventMessage::Close(id) => self.request_close(id),
            EventMessage::Event(event, status, id) => match event {
                Event::Window(window::Event::Opened {
                    position: Some(origin),
                    size,
                }) => {
                    self.frames.insert(id, Rectangle::new(origin, size));
                    Task::none()
                }
                Event::Window(window::Event::Moved(position)) => {
                    if let Some(frame) = self.frames.get_mut(&id) {
                        frame.x = position.x;
                        frame.y = position.y;
                    }
                    Task::none()
                }
                Event::Window(window::Event::Resized(size)) => {
                    if let Some(frame) = self.frames.get_mut(&id) {
                        frame.width = size.width;
                        frame.height = size.height;
                    }
                    if let Some(app) = self.windows.get_mut(&id) {
                        app.window_width = size.width;
                        app.sync_horizontal_cursor()
                            .map(move |message| EventMessage::Window(id, message))
                    } else {
                        Task::none()
                    }
                }
                Event::Keyboard(iced::keyboard::Event::KeyPressed { key, modifiers, .. }) => self
                    .dispatch(
                        id,
                        Message::KeyPressed(key, modifiers, status == event::Status::Captured),
                    ),
                Event::Mouse(mouse::Event::CursorMoved { position }) => {
                    if let Some(a) = self.windows.get_mut(&id) {
                        a.pointer = position;
                    }
                    Task::none()
                }
                Event::Window(window::Event::Focused) => {
                    if self.focused != id {
                        self.focused = id;
                        self.changed();
                    }
                    self.windows
                        .get(&id)
                        .map(|app| {
                            let task = if app.context.is_some() || app.close_confirmation.is_some()
                            {
                                app.unfocus_editor()
                            } else if app.panel != crate::Panel::None {
                                app.focus_panel()
                            } else {
                                app.focus_editor()
                            };
                            task.map(move |m| EventMessage::Window(id, m))
                        })
                        .unwrap_or_else(Task::none)
                }
                Event::Window(window::Event::FileDropped(path)) => {
                    self.dispatch(id, Message::FileDropped(path))
                }
                _ => Task::none(),
            },
            EventMessage::Tick => {
                for app in self.windows.values_mut() {
                    app.expire_toast();
                }
                #[cfg(target_os = "macos")]
                {
                    if crate::macos::take_quit_request() {
                        return self.quit();
                    }
                    let mut tasks = Vec::new();
                    for action in crate::macos::take_menu_actions() {
                        let id = self.focused;
                        if let Some(app) = self.windows.get(&id)
                            && app.close_confirmation.is_none()
                        {
                            let message = app.menu_message(action);
                            tasks.push(self.dispatch(id, message));
                        }
                    }
                    while let Some(path) = crate::macos::try_recv() {
                        tasks.push(self.dispatch(self.focused, Message::FileDropped(path)));
                    }
                    if !tasks.is_empty() {
                        return Task::batch(tasks);
                    }
                }
                if self.closing.is_some()
                    || self.quitting
                    || self.last_change.elapsed() >= Duration::from_millis(400)
                {
                    self.persist()
                } else {
                    Task::none()
                }
            }
            EventMessage::Saved(revision, result) => {
                self.saving = false;
                match result {
                    Ok(()) => {
                        self.persisted = revision;
                        if self.revision != revision && (self.quitting || self.closing.is_some()) {
                            self.persist()
                        } else if self.quitting {
                            iced::exit()
                        } else if let Some(id) = self.closing {
                            self.finish_close(id)
                        } else {
                            Task::none()
                        }
                    }
                    Err(error) => {
                        self.closing = None;
                        self.quitting = false;
                        self.last_change = Instant::now() + Duration::from_secs(2);
                        for app in self.windows.values_mut() {
                            app.status_notification = Some(format!(
                                "Could not save drafts: {error}. Your windows remain open."
                            ));
                        }
                        Task::none()
                    }
                }
            }
        }
    }
    pub fn subscription(&self) -> Subscription<EventMessage> {
        Subscription::batch([
            window::close_requests().map(EventMessage::Close),
            iced::time::every(Duration::from_millis(150)).map(|_| EventMessage::Tick),
            event::listen_with(|e, status, id| Some(EventMessage::Event(e, status, id))),
        ])
    }
}

fn settings() -> window::Settings {
    window::Settings {
        size: Size::new(1160.0, 760.0),
        min_size: Some(Size::new(820.0, 540.0)),
        position: window::Position::Centered,
        exit_on_close_request: false,
        #[cfg(target_os = "macos")]
        platform_specific: window::settings::PlatformSpecific {
            title_hidden: true,
            titlebar_transparent: true,
            fullsize_content_view: true,
        },
        ..Default::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn desktop() -> Desktop {
        let id = window::Id::unique();
        let mut tab = Tab::new_empty(0);
        tab.on_edit(iced::widget::text_editor::Action::Edit(
            iced::widget::text_editor::Edit::Insert('x'),
        ));
        Desktop {
            windows: BTreeMap::from([(id, App::from_tabs(vec![tab], 0, None))]),
            frames: BTreeMap::from([(id, Rectangle::new(Point::ORIGIN, Size::new(800.0, 600.0)))]),
            parked: vec![],
            prefs: crate::prefs::Prefs::default(),
            focused: id,
            revision: 1,
            persisted: 0,
            saving: false,
            blocked: false,
            last_change: Instant::now(),
            closing: None,
            quitting: false,
            _lock: None,
        }
    }
    #[test]
    fn detached_unsaved_document_keeps_identity_content_and_history() {
        let mut d = desktop();
        let source = d.focused;
        let id = d.windows[&source].tabs[0].id.clone();
        d.windows
            .get_mut(&source)
            .unwrap()
            .tabs
            .push(Tab::new_empty(1));
        let _ = d.detach(source, id.clone(), None);
        assert_eq!(d.windows.len(), 2);
        let tab = d
            .windows
            .values()
            .flat_map(|a| a.tabs.iter())
            .find(|t| t.id == id)
            .unwrap();
        assert_eq!(tab.content.text(), "x");
        assert!(tab.is_dirty);
        let app = d
            .windows
            .values_mut()
            .find(|a| a.tabs.iter().any(|t| t.id == id))
            .unwrap();
        app.tabs[0].undo();
        assert_eq!(app.tabs[0].content.text(), "");
    }
    #[test]
    fn detaching_a_lone_tab_without_a_target_keeps_its_window() {
        let mut d = desktop();
        let source = d.focused;
        let id = d.windows[&source].tabs[0].id.clone();
        let _ = d.detach(source, id, Some(Point::new(100.0, 300.0)));
        assert_eq!(d.windows.len(), 1);
        assert_eq!(d.windows[&source].tabs.len(), 1);
    }
    #[test]
    fn dropping_a_lone_tab_on_another_window_merges_and_closes_the_source() {
        let mut d = desktop();
        let target = d.focused;
        let source = window::Id::unique();
        let tab = Tab::new_empty(7);
        let id = tab.id.clone();
        d.windows.insert(source, App::from_tabs(vec![tab], 0, None));
        d.frames.insert(
            source,
            Rectangle::new(Point::new(900.0, 0.0), Size::new(400.0, 300.0)),
        );
        // Released over the target's tab bar, left of its first tab's centre.
        let _ = d.detach(source, id.clone(), Some(Point::new(-880.0, 10.0)));
        assert!(!d.windows.contains_key(&source));
        assert!(!d.frames.contains_key(&source));
        let app = &d.windows[&target];
        assert_eq!(app.tabs[0].id, id);
        assert_eq!(app.active_tab, 0);
        assert_eq!(d.focused, target);
    }
    #[test]
    fn write_failure_cancels_close_and_quit_for_all_windows() {
        let mut d = desktop();
        d.closing = Some(d.focused);
        d.quitting = true;
        d.saving = true;
        let _ = d.update(EventMessage::Saved(1, Err("disk full".into())));
        assert!(d.closing.is_none());
        assert!(!d.quitting);
        assert_eq!(d.windows.len(), 1);
        assert!(
            d.windows[&d.focused]
                .status_notification
                .as_ref()
                .unwrap()
                .contains("disk full")
        );
    }
    #[test]
    fn closing_one_window_preserves_all_drafts_in_next_snapshot() {
        let mut d = desktop();
        let source = d.focused;
        let id = d.windows[&source].tabs[0].id.clone();
        d.windows
            .get_mut(&source)
            .unwrap()
            .tabs
            .push(Tab::new_empty(1));
        let _ = d.detach(source, id.clone(), None);
        let _ = d.finish_close(source);
        let root = std::env::temp_dir().join(crate::storage::unique_id());
        let drafts = root.join("drafts");
        d.snapshot().save_to(&root, &drafts).unwrap();
        let (tabs, _) = session::load_session_from(&root, &drafts).unwrap().unwrap();
        assert_eq!(tabs.len(), 1);
        assert_eq!(tabs.iter().filter(|t| t.id == id).count(), 1);
        assert_eq!(
            tabs.iter().find(|t| t.id == id).unwrap().content.text(),
            "x"
        );
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn file_open_in_another_window_is_focused_without_duplicate_buffer() {
        let mut d = desktop();
        let source = d.focused;
        let path = std::env::temp_dir().join(crate::storage::unique_id());
        std::fs::write(&path, "disk").unwrap();
        d.windows.get_mut(&source).unwrap().tabs[0].path = Some(path.clone());
        let id = d.windows[&source].tabs[0].id.clone();
        d.windows
            .get_mut(&source)
            .unwrap()
            .tabs
            .push(Tab::new_empty(1));
        let _ = d.detach(source, id, None);
        let _ = d.dispatch(source, Message::FileDropped(path.clone()));
        let count = d
            .windows
            .values()
            .flat_map(|a| a.tabs.iter())
            .filter(|t| t.path.as_ref() == Some(&path))
            .count();
        assert_eq!(count, 1);
        assert_ne!(d.focused, source);
        std::fs::remove_file(path).unwrap();
    }
}
