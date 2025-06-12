use crate::cursor::CustomCursorSource;
use crate::error::EventLoopError;
use crate::event::Event;
use crate::event_loop::{ControlFlow, DeviceEvents, EventLoopClosed};
use crate::platform_impl::emscripten::event_hub::EventHub;
use crate::platform_impl::{CustomCursorFuture, PlatformCustomCursor};
use crate::window::Theme;
use std::collections::vec_deque::IntoIter as VecDequeIter;
use std::collections::VecDeque;
use std::marker::PhantomData;
use std::sync::mpsc;
use std::sync::mpsc::{Receiver, SendError, Sender};

#[derive(Default, Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub(crate) struct PlatformSpecificEventLoopAttributes {}

pub struct EventLoop<T> {
    elw: crate::event_loop::ActiveEventLoop,
    user_event_sender: Sender<T>,
    user_event_receiver: Receiver<T>,
}

pub struct EventLoopProxy<T: 'static> {
    sender: Sender<T>,
}

impl<T: 'static> EventLoopProxy<T> {
    pub fn send_event(&self, event: T) -> Result<(), EventLoopClosed<T>> {
        self.sender.send(event).map_err(|SendError(event)| EventLoopClosed(event))?;
        Ok(())
    }
}

impl<T> Clone for EventLoopProxy<T> {
    fn clone(&self) -> Self {
        Self { sender: self.sender.clone() }
    }
}


impl<T: 'static> EventLoop<T> {
    pub fn new(
        _attrs: &mut PlatformSpecificEventLoopAttributes,
    ) -> Result<EventLoop<T>, EventLoopError> {
        let (user_event_sender, user_event_receiver) = mpsc::channel();
        let elw =
            crate::event_loop::ActiveEventLoop { p: ActiveEventLoop::new(), _marker: PhantomData };
        Ok(EventLoop { elw, user_event_sender, user_event_receiver })
    }

    pub fn run<F: 'static>(self, mut event_handler: F) -> Result<(), EventLoopError>
    where
        F: FnMut(Event<T>, &crate::event_loop::ActiveEventLoop),
    {
        // self.interrupted.store(false, Ordering::Relaxed);
        event_handler(Event::Resumed, &self.elw);

        let target = self.elw;
        let mut handler: Box<dyn FnMut(Event<T>)> = Box::new(move |event| {
            event_handler(event, &target);
        });
        let user_event_receiver = self.user_event_receiver;
        // TODO: handle control flow
        crate::platform_impl::emscripten::set_main_loop_callback(move || {
            EventHub::poll_events(|e| handler(e));
            while let Ok(e) = user_event_receiver.try_recv() {
                handler(Event::UserEvent(e));
            }
            ::std::thread::sleep(::std::time::Duration::from_millis(5));
            //TODO support interrupted?
            // if self.interrupted.load(Ordering::Relaxed) {
            //     unsafe { ffi::emscripten_cancel_main_loop(); }
            // }
        });
        Ok(())
    }

    #[inline]
    pub fn create_proxy(&self) -> EventLoopProxy<T> {
        EventLoopProxy { sender: self.user_event_sender.clone() }
    }

    pub fn window_target(&self) -> &crate::event_loop::ActiveEventLoop {
        &self.elw
    }
}

pub struct ActiveEventLoop {}

impl ActiveEventLoop {
    pub fn new() -> Self {
        Self {}
    }

    #[allow(unused)]
    pub fn run(
        &self,
        _event_handler: Box<crate::platform_impl::emscripten::EventHandler>,
        _event_loop_recreation: bool,
    ) {
        //TODO impl
    }

    pub fn create_custom_cursor(&self, _source: CustomCursorSource) -> crate::cursor::CustomCursor {
        //TODO impl
        crate::cursor::CustomCursor { inner: PlatformCustomCursor {} }
    }

    pub fn create_custom_cursor_async(&self, _source: CustomCursorSource) -> CustomCursorFuture {
        CustomCursorFuture {}
    }

    pub fn available_monitors(
        &self,
    ) -> VecDequeIter<crate::platform_impl::emscripten::monitor::MonitorHandle> {
        VecDeque::new().into_iter()
    }

    pub fn primary_monitor(&self) -> Option<crate::platform_impl::emscripten::monitor::MonitorHandle> {
        None
    }

    #[cfg(feature = "rwh_05")]
    #[inline]
    pub fn raw_display_handle_rwh_05(&self) -> rwh_05::RawDisplayHandle {
        rwh_05::RawDisplayHandle::Web(rwh_05::WebDisplayHandle::empty())
    }

    #[cfg(feature = "rwh_06")]
    #[inline]
    pub fn raw_display_handle_rwh_06(
        &self,
    ) -> Result<rwh_06::RawDisplayHandle, rwh_06::HandleError> {
        Ok(rwh_06::RawDisplayHandle::Web(rwh_06::WebDisplayHandle::new()))
    }

    pub fn listen_device_events(&self, _allowed: DeviceEvents) {
        //TODO impl listen device events
    }

    pub fn system_theme(&self) -> Option<Theme> {
        //TODO impl system theme
        None
    }

    pub(crate) fn set_control_flow(&self, _control_flow: ControlFlow) {
        //TODO impl
    }

    pub(crate) fn control_flow(&self) -> ControlFlow {
        //TODO impl
        ControlFlow::Wait
    }

    pub(crate) fn exit(&self) {
        //TODO impl
    }

    pub(crate) fn exiting(&self) -> bool {
        //TODO impl
        false
    }

    pub(crate) fn owned_display_handle(
        &self,
    ) -> crate::platform_impl::emscripten::window_target::OwnedDisplayHandle {
        crate::platform_impl::emscripten::window_target::OwnedDisplayHandle
    }

    pub fn query_pointer(
        &self,
        _device_id: crate::platform_impl::emscripten::event::DeviceId,
    ) -> Option<(f32, f32)> {
        None
    }
}
