//! Log Event Manager

use crate::sync::fifo::AsyncEventQueue;
use crate::system::System;
use crate::*;
use core::mem::MaybeUninit;
use core::pin::Pin;
use core::ptr::{addr_of, addr_of_mut};
use futures_util::Future;

#[macro_export]
macro_rules! notify {
    ($icon:expr, $($arg:tt)*) => {
        let mut sb = megstd::string::Sb255::new();
        write!(sb, $($arg)*).unwrap();
        utils::EventManager::notify_simple_message($icon, sb.as_str());
    };
}

pub struct Log;

impl Log {
    #[inline]
    pub const fn new() -> Self {
        Self {}
    }
}

impl Write for Log {
    #[inline]
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        EventManager::system_log(s);
        Ok(())
    }
}

static mut EVENT_MANAGER: MaybeUninit<EventManager> = MaybeUninit::uninit();

pub struct EventManager {
    message_queue: AsyncEventQueue<SimpleMessagePayload>,
}

impl EventManager {
    fn new() -> Self {
        Self {
            message_queue: AsyncEventQueue::new(1000),
        }
    }

    pub fn init() {
        assert_call_once!();

        unsafe {
            (&mut *addr_of_mut!(EVENT_MANAGER)).write(Self::new());
        }
    }

    #[inline]
    fn shared<'a>() -> &'a Self {
        unsafe { &*(&*addr_of!(EVENT_MANAGER)).as_ptr() }
    }

    pub fn system_log(s: &str) {
        let _ = write!(System::log(), "{}", s);
    }

    pub fn notify_simple_message(icon: r::Icons, message: &str) {
        let shared = Self::shared();
        let payload = SimpleMessagePayload::new(icon, message);
        shared.message_queue.post(payload).unwrap();
    }

    pub fn monitor_notification() -> Pin<Box<dyn Future<Output = Option<SimpleMessagePayload>>>> {
        let shared = Self::shared();
        Box::pin(shared.message_queue.wait_event())
    }
}

#[derive(Debug, Clone)]
pub struct SimpleMessagePayload {
    icon: r::Icons,
    message: String,
}

impl SimpleMessagePayload {
    #[inline]
    pub fn new(icon: r::Icons, message: &str) -> Self {
        Self {
            icon,
            message: message.to_string(),
        }
    }

    #[inline]
    pub const fn icon(&self) -> r::Icons {
        self.icon
    }

    #[inline]
    pub fn message<'a>(&'a self) -> &'a str {
        self.message.as_str()
    }
}
