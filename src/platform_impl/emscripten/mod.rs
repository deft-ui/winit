mod cursor;
mod event;
mod event_hub;
mod event_loop;
mod ffi;
mod keyboard;
mod window;
mod window_target;
mod monitor;

pub use cursor::*;
pub use event::*;
pub use event_loop::*;
pub use event_loop::{ActiveEventLoop, EventLoop};
pub use monitor::*;
use std::cell::RefCell;
use std::fmt::{Display, Formatter};
use std::str;
pub use window::*;
pub use window_target::*;

use crate::event::Event;

pub(crate) use crate::icon::NoIcon as PlatformIcon;
use deft_emscripten_sys as bindings;
use libc::c_char;
use crate::platform_impl::emscripten::event_hub::EventHub;

const DOCUMENT_NAME: &'static str = "1\0";
const WINDOW_NAME: &'static str = "2\0";

const BODY_NAME: &'static str = "body\0";

fn get_hidpi_factor() -> f64 {
    unsafe { bindings::emscripten_get_device_pixel_ratio() }
}


pub(super) type EventHandler = dyn FnMut(Event<()>);

struct MainLoopCallback {
    callback: Box<dyn FnMut()>,
}

impl MainLoopCallback {
    pub fn new(callback: Box<dyn FnMut()>) -> MainLoopCallback {
        Self { callback }
    }
    pub fn empty() -> Self {
        MainLoopCallback { callback: Box::new(|| {}) }
    }
}

// Used to assign a callback to emscripten main loop
thread_local!(static MAIN_LOOP_CALLBACK: RefCell<MainLoopCallback> = RefCell::new(MainLoopCallback::empty()));

// Used to assign a callback to emscripten main loop
pub fn set_main_loop_callback<F: 'static>(callback: F)
where
    F: FnMut(),
{
    MAIN_LOOP_CALLBACK.with_borrow_mut(move |log| {
        *log = MainLoopCallback::new(Box::new(callback));
    });

    unsafe {
        bindings::emscripten_set_main_loop(Some(wrapper::<F>), 0, true);
    }

    unsafe extern "C" fn wrapper<F>()
    where
        F: FnMut(),
    {
        MAIN_LOOP_CALLBACK.with_borrow_mut(|z| {
            (z.callback)();
        });
    }
}

fn em_try(res: ffi::EMSCRIPTEN_RESULT) -> Result<(), String> {
    match res {
        ffi::EMSCRIPTEN_RESULT_SUCCESS | ffi::EMSCRIPTEN_RESULT_DEFERRED => Ok(()),
        r @ _ => Err(error_to_str(r).to_string()),
    }
}


#[derive(Debug)]
pub struct OsError(pub String);

impl Display for OsError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

fn error_to_str(code: ffi::EMSCRIPTEN_RESULT) -> &'static str {
    match code {
        ffi::EMSCRIPTEN_RESULT_SUCCESS | ffi::EMSCRIPTEN_RESULT_DEFERRED => {
            "Internal error in the library (success detected as failure)"
        },

        ffi::EMSCRIPTEN_RESULT_NOT_SUPPORTED => "Not supported",
        ffi::EMSCRIPTEN_RESULT_FAILED_NOT_DEFERRED => "Failed not deferred",
        ffi::EMSCRIPTEN_RESULT_INVALID_TARGET => "Invalid target",
        ffi::EMSCRIPTEN_RESULT_UNKNOWN_TARGET => "Unknown target",
        ffi::EMSCRIPTEN_RESULT_INVALID_PARAM => "Invalid parameter",
        ffi::EMSCRIPTEN_RESULT_FAILED => "Failed",
        ffi::EMSCRIPTEN_RESULT_NO_DATA => "No data",

        _ => "Undocumented error",
    }
}


#[no_mangle]
pub extern "C" fn winit_emscripten_send_input(text: *const c_char) {
    let text = unsafe { std::ffi::CStr::from_ptr(text).to_string_lossy() };
    EventHub::send_event(Event::WindowEvent {
        window_id: crate::window::WindowId(WindowId(0)),
        event: crate::event::WindowEvent::Ime(crate::event::Ime::Commit(text.to_string())),
    });
}