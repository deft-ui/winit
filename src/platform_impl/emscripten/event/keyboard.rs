use std::ffi::CStr;
use smol_str::SmolStr;
use crate::event::{ElementState, Event, KeyEvent};
use crate::event::WindowEvent::KeyboardInput;
use crate::keyboard::{Key, KeyLocation, NamedKey, PhysicalKey};
use crate::platform_impl::emscripten::bindings::EmscriptenKeyboardEvent;
use crate::platform_impl::emscripten::event::{DeviceId, KeyEventExtra};
use crate::platform_impl::emscripten::event_hub::EventHub;
use crate::platform_impl::emscripten::ffi;
use crate::platform_impl::WindowId;

pub fn key_text(key: &Key) -> Option<SmolStr> {
    match &key {
        Key::Character(text) => Some(text.clone()),
        Key::Named(NamedKey::Tab) => Some(SmolStr::new("\t")),
        Key::Named(NamedKey::Enter) => Some(SmolStr::new("\r")),
        Key::Named(NamedKey::Space) => Some(SmolStr::new(" ")),
        _ => None,
    }
        .map(SmolStr::new)
}

pub extern "C" fn keyboard_callback(
    event_type: ::core::ffi::c_int,
    event: *const EmscriptenKeyboardEvent,
    _user_data: *mut ::core::ffi::c_void,
) -> bool {
    unsafe {
        let event = &*event;
        let code = CStr::from_ptr(event.code.as_ptr()).to_str().unwrap();
        let key = CStr::from_ptr(event.key.as_ptr()).to_str().unwrap();
        let physical_key = PhysicalKey::from_key_code_attribute_value(code);
        let logical_key = Key::from_key_attribute_value(key);
        let text = key_text(&logical_key);
        let state = match event_type {
            ffi::EMSCRIPTEN_EVENT_KEYDOWN => ElementState::Pressed,
            ffi::EMSCRIPTEN_EVENT_KEYUP => ElementState::Released,
            _ => return false,
        };
        EventHub::send_event(Event::WindowEvent {
            window_id: crate::window::WindowId(WindowId(0)),
            event: KeyboardInput {
                device_id: crate::event::DeviceId(DeviceId),
                event: KeyEvent {
                    physical_key,
                    logical_key,
                    text,
                    //TODO fix location?
                    location: KeyLocation::Standard,
                    state,
                    repeat: false,
                    platform_specific: KeyEventExtra,
                },
                is_synthetic: false,
            },
        });
    }
    false
}
