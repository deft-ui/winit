use dpi::{LogicalPosition};
use std::os::raw::{c_int, c_void};
use crate::event;
use crate::event::{ElementState, Event, MouseButton, WindowEvent};
use crate::platform_impl::emscripten::event::DeviceId;
use crate::platform_impl::emscripten::event_hub::EventHub;
use crate::platform_impl::emscripten::ffi;
pub use crate::platform_impl::emscripten::window::*;
use crate::platform_impl::platform::{bindings, get_hidpi_factor};

pub extern "C" fn mouse_callback(
    event_type: c_int,
    event: *const bindings::EmscriptenMouseEvent,
    _: *mut c_void,
) -> bool {
    unsafe {
        match event_type {
            ffi::EMSCRIPTEN_EVENT_MOUSEMOVE => {
                let position =
                    LogicalPosition::new((*event).clientX as f64, (*event).clientY as f64)
                        .to_physical(get_hidpi_factor());
                let root_position = position.clone();
                EventHub::send_event(Event::WindowEvent {
                    window_id: crate::window::WindowId(WindowId(0)),
                    event: WindowEvent::CursorMoved {
                        device_id: event::DeviceId(DeviceId),
                        position,
                        root_position,
                    },
                });
                EventHub::send_event(Event::DeviceEvent {
                    device_id: event::DeviceId(DeviceId),
                    event: event::DeviceEvent::MouseMotion {
                        delta: ((*event).movementX as f64, (*event).movementY as f64),
                    },
                });
            },
            ffi::EMSCRIPTEN_EVENT_MOUSEDOWN | ffi::EMSCRIPTEN_EVENT_MOUSEUP => {
                let button = match (*event).button {
                    0 => MouseButton::Left,
                    1 => MouseButton::Middle,
                    2 => MouseButton::Right,
                    other => MouseButton::Other(other as u16),
                };
                let state = match event_type {
                    ffi::EMSCRIPTEN_EVENT_MOUSEDOWN => ElementState::Pressed,
                    ffi::EMSCRIPTEN_EVENT_MOUSEUP => ElementState::Released,
                    _ => unreachable!(),
                };
                EventHub::send_event(Event::WindowEvent {
                    window_id: crate::window::WindowId(WindowId(0)),
                    event: WindowEvent::MouseInput {
                        device_id: event::DeviceId(DeviceId),
                        state,
                        button,
                    },
                });
            },
            _ => {},
        }
    }
    false
}
