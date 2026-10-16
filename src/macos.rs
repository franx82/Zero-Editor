use std::ffi::CStr;
use std::os::raw::c_char;
use std::path::PathBuf;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
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
    fn style_macos_titlebar();
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
