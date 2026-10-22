use crate::commands::Command;
use crate::menu;
use std::ffi::{CStr, CString};
use std::os::raw::c_char;
use std::path::PathBuf;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
use std::sync::mpsc::{Receiver, Sender, channel};
static QUIT_REQUESTED: AtomicBool = AtomicBool::new(false);
extern "C" fn on_quit() {
    QUIT_REQUESTED.store(true, Ordering::SeqCst);
}
pub fn take_quit_request() -> bool {
    QUIT_REQUESTED.swap(false, Ordering::SeqCst)
}

static CHANNEL: Mutex<Option<(Sender<PathBuf>, Receiver<PathBuf>)>> = Mutex::new(None);

extern "C" fn on_file_open(path_ptr: *const c_char) {
    if path_ptr.is_null() {
        return;
    }
    let c_str = unsafe { CStr::from_ptr(path_ptr) };
    if let Ok(path_str) = c_str.to_str() {
        let path = PathBuf::from(path_str);
        if let Ok(guard) = CHANNEL.lock()
            && let Some((tx, _)) = guard.as_ref()
        {
            let _ = tx.send(path);
        }
    }
}

unsafe extern "C" {
    fn register_macos_file_callback(cb: extern "C" fn(*const c_char));
    fn register_macos_quit_callback(cb: extern "C" fn());
    fn register_macos_menu_callbacks(action: extern "C" fn(i32), state: extern "C" fn(i32) -> i32);
    fn style_macos_titlebar();
    fn zero_menu_begin(title: *const c_char);
    fn zero_menu_item(title: *const c_char, key: *const c_char, modifiers: u32, tag: i32);
    fn zero_menu_separator();
    fn zero_menu_native(name: *const c_char);
    fn zero_menu_install();
    fn zero_note_recent(path: *const c_char);
}

static MENU_ACTIONS: Mutex<Vec<menu::Action>> = Mutex::new(Vec::new());
/// Checkmark state for toggle items: bit 0 wrap, 1 whitespace, 2 highlight.
static MENU_CHECKS: AtomicU8 = AtomicU8::new(0);

extern "C" fn on_menu_action(tag: i32) {
    if let Some(menu::Entry::Item(_, _, _, action)) = menu::MENU.get(tag as usize)
        && let Ok(mut queue) = MENU_ACTIONS.lock()
    {
        queue.push(*action);
    }
}

extern "C" fn menu_state(tag: i32) -> i32 {
    let Some(menu::Entry::Item(_, _, _, action)) = menu::MENU.get(tag as usize) else {
        return -1;
    };
    let bit = match action {
        menu::Action::Run(Command::WordWrap) => 0,
        menu::Action::Run(Command::Whitespace) => 1,
        menu::Action::Run(Command::Highlight) => 2,
        _ => return -1,
    };
    i32::from(MENU_CHECKS.load(Ordering::Relaxed) & (1 << bit) != 0)
}

/// Installs the menu bar once AppKit has created its application menu.
pub fn install_menu() {
    static INSTALLED: AtomicBool = AtomicBool::new(false);
    if INSTALLED.swap(true, Ordering::SeqCst) {
        return;
    }
    let c = |s: &str| CString::new(s).unwrap_or_default();
    unsafe {
        register_macos_menu_callbacks(on_menu_action, menu_state);
        for (tag, entry) in menu::MENU.iter().enumerate() {
            match entry {
                menu::Entry::Menu(title) => zero_menu_begin(c(title).as_ptr()),
                menu::Entry::Item(title, key, modifiers, _) => {
                    zero_menu_item(c(title).as_ptr(), c(key).as_ptr(), *modifiers, tag as i32)
                }
                menu::Entry::Native(name) => zero_menu_native(c(name).as_ptr()),
                menu::Entry::Separator => zero_menu_separator(),
            }
        }
        zero_menu_install();
    }
}

pub fn set_menu_checks(prefs: &crate::prefs::Prefs) {
    let bits = u8::from(prefs.word_wrap)
        | u8::from(prefs.show_whitespace) << 1
        | u8::from(prefs.highlight) << 2;
    MENU_CHECKS.store(bits, Ordering::Relaxed);
}

pub fn take_menu_actions() -> Vec<menu::Action> {
    MENU_ACTIONS
        .lock()
        .map(|mut queue| std::mem::take(&mut *queue))
        .unwrap_or_default()
}

/// Adds a file to the system's recent documents (File ▸ Open Recent, Dock menu).
pub fn note_recent(path: &std::path::Path) {
    if let Ok(path) = CString::new(path.as_os_str().as_encoded_bytes()) {
        unsafe { zero_note_recent(path.as_ptr()) }
    }
}

pub fn style_titlebar() {
    unsafe { style_macos_titlebar() }
}

pub fn init() {
    let (tx, rx) = channel();
    if let Ok(mut guard) = CHANNEL.lock() {
        *guard = Some((tx, rx));
    }
    unsafe {
        register_macos_quit_callback(on_quit);
        register_macos_file_callback(on_file_open);
    }
}

pub fn take_initial_files() -> Vec<PathBuf> {
    let mut paths = Vec::new();
    while let Some(path) = try_recv() {
        paths.push(path);
    }
    paths
}

pub fn try_recv() -> Option<PathBuf> {
    if let Ok(guard) = CHANNEL.lock()
        && let Some((_, rx)) = guard.as_ref()
    {
        return rx.try_recv().ok();
    }
    None
}
