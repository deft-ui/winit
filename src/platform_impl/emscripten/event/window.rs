use crate::event::{Event, WindowEvent};
use crate::platform_impl::emscripten::bindings::EmscriptenUiEvent;
use crate::platform_impl::emscripten::event_hub::EventHub;
pub use crate::platform_impl::emscripten::window::*;
use crate::platform_impl::platform::get_hidpi_factor;
use dpi::{LogicalSize};

pub extern "C" fn window_resize_callback(
    _event_type: ::core::ffi::c_int,
    ui_event: *const EmscriptenUiEvent,
    _user_data: *mut ::core::ffi::c_void,
) -> bool {
    let (width, height) = unsafe {
        let e = &*ui_event;
        (e.windowInnerWidth as u32, e.windowInnerHeight as u32)
    };
    let size = LogicalSize { width, height }.to_physical(get_hidpi_factor());
    let ev = Event::WindowEvent {
        window_id: crate::window::WindowId(WindowId(0)),
        event: WindowEvent::Resized(size),
    };
    EventHub::send_event(ev);
    false
}
