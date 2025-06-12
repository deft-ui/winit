use crate::event::Event;
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

thread_local! {
    static EVENT_HUB: EventHub = EventHub {
        queue: Arc::new(Mutex::new(VecDeque::new())),
    }
}

#[derive(Debug, Clone)]
pub struct EventHub {
    pub queue: Arc<Mutex<VecDeque<Event<()>>>>,
}

impl EventHub {
    pub fn send_event(event: Event<()>) {
        EVENT_HUB.with(|hub| {
            let mut queue = hub.queue.lock().unwrap();
            queue.push_back(event);
        })
    }

    pub fn poll_events<T: 'static, F>(mut callback: F)
    where
        F: FnMut(Event<T>),
    {
        EVENT_HUB.with(move |hub| {
            let mut list = hub.queue.lock().unwrap();
            while let Some(ev) = list.pop_front() {
                callback(ev.map_nonuser_event().unwrap());
            }
        });
    }
}
