use std::ffi::CStr;
use std::os::raw::c_char;
use std::path::PathBuf;
use std::sync::Mutex;
use std::sync::mpsc::{channel, Receiver, Sender};

static CHANNEL: Mutex<Option<(Sender<PathBuf>, Receiver<PathBuf>)>> = Mutex::new(None);
static INITIAL_PATHS: Mutex<Vec<PathBuf>> = Mutex::new(Vec::new());

extern "C" fn on_file_open(path_ptr: *const c_char) {
    if path_ptr.is_null() {
        return;
    }
    let c_str = unsafe { CStr::from_ptr(path_ptr) };
    if let Ok(path_str) = c_str.to_str() {
        let path = PathBuf::from(path_str);
        if let Ok(guard) = CHANNEL.lock() {
            if let Some((tx, _)) = guard.as_ref() {
                let _ = tx.send(path.clone());
            }
        }
        if let Ok(mut initial) = INITIAL_PATHS.lock() {
            initial.push(path);
        }
    }
}

unsafe extern "C" {
    fn register_macos_file_callback(cb: extern "C" fn(*const c_char));
}

pub fn init() {
    let (tx, rx) = channel();
    if let Ok(mut guard) = CHANNEL.lock() {
        *guard = Some((tx, rx));
    }
    unsafe {
        register_macos_file_callback(on_file_open);
    }
}

pub fn take_initial_files() -> Vec<PathBuf> {
    if let Ok(mut initial) = INITIAL_PATHS.lock() {
        std::mem::take(&mut *initial)
    } else {
        Vec::new()
    }
}

pub fn try_recv() -> Option<PathBuf> {
    if let Ok(guard) = CHANNEL.lock() {
        if let Some((_, rx)) = guard.as_ref() {
            return rx.try_recv().ok();
        }
    }
    None
}
