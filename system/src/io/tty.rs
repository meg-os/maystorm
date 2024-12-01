//! TeleTypewriter

use crate::*;
use core::cell::UnsafeCell;
use core::future::Future;
use core::pin::Pin;
use core::ptr::addr_of;
use core::task::{Context, Poll};

pub trait TtyWrite: Write {
    fn reset(&mut self) -> Result<(), TtyError>;

    fn dims(&self) -> (u32, u32);

    fn cursor_position(&self) -> (u32, u32);

    fn set_cursor_position(&mut self, x: u32, y: u32);

    fn is_cursor_enabled(&self) -> bool;

    fn set_cursor_enabled(&mut self, enabled: bool) -> bool;

    fn set_attribute(&mut self, attribute: u8);

    fn attributes(&self) -> u8 {
        0
    }
}

pub trait TtyRead {
    fn read_async(&self) -> Pin<Box<dyn Future<Output = TtyReadResult> + '_>>;
}

pub trait Tty: TtyWrite + TtyRead {}

impl dyn Tty {
    pub async fn read_line_async(&mut self, max_length: usize) -> Result<String, TtyError> {
        let mut buffer: Vec<char> = Vec::with_capacity(max_length);
        loop {
            self.set_cursor_enabled(true);
            match self.read_async().await {
                Ok(c) => {
                    self.set_cursor_enabled(false);
                    match c {
                        '\r' | '\n' => {
                            self.write_str("\r\n").unwrap();
                            break;
                        }
                        '\x03' => return Err(TtyError::EndOfStream),
                        '\x08' => match buffer.pop() {
                            Some(c) => {
                                if c < ' ' {
                                    self.write_str("\x08\x08  \x08\x08").unwrap();
                                } else {
                                    self.write_str("\x08 \x08").unwrap();
                                }
                            }
                            None => (),
                        },
                        _ => {
                            if buffer.len() < max_length {
                                if c < ' ' {
                                    // Control char
                                    self.write_char('^').unwrap();
                                    self.write_char((c as u8 | 0x40) as char).unwrap();
                                    buffer.push(c);
                                } else if c < '\x7F' {
                                    // Printable ascii
                                    self.write_char(c).unwrap();
                                    buffer.push(c);
                                } else {
                                    // TODO:
                                }
                            }
                        }
                    }
                }
                Err(TtyError::EndOfStream) => return Err(TtyError::EndOfStream),
                Err(_) => (),
            }
        }
        Ok(buffer.as_slice().iter().clone().collect::<String>())
    }
}

#[derive(Debug)]
pub enum TtyError {
    NotReady,
    DeviceError,
    EndOfStream,
}

pub type TtyReadResult = Result<char, TtyError>;

// Null is singleton
static mut NULL_TTY: UnsafeCell<NullTty> = UnsafeCell::new(NullTty::new());

/// Null Tty
pub struct NullTty;

impl NullTty {
    #[inline]
    const fn new() -> Self {
        Self {}
    }

    #[inline]
    pub fn null<'a>() -> &'a mut dyn Tty {
        unsafe { &mut *(&*addr_of!(NULL_TTY)).get() }
    }
}

impl Write for NullTty {
    fn write_str(&mut self, _s: &str) -> core::fmt::Result {
        Ok(())
    }
}

impl TtyWrite for NullTty {
    fn reset(&mut self) -> Result<(), TtyError> {
        Ok(())
    }

    fn dims(&self) -> (u32, u32) {
        (0, 0)
    }

    fn cursor_position(&self) -> (u32, u32) {
        (0, 0)
    }

    fn set_cursor_position(&mut self, _x: u32, _y: u32) {}

    fn is_cursor_enabled(&self) -> bool {
        false
    }

    fn set_cursor_enabled(&mut self, _enabled: bool) -> bool {
        false
    }

    fn set_attribute(&mut self, _attribute: u8) {}
}

impl TtyRead for NullTty {
    fn read_async(
        &self,
    ) -> core::pin::Pin<Box<dyn core::future::Future<Output = TtyReadResult> + '_>> {
        Box::pin(NullReader {})
    }
}

impl Tty for NullTty {}

struct NullReader {}

impl Future for NullReader {
    type Output = TtyReadResult;

    fn poll(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Self::Output> {
        Poll::Ready(Err(TtyError::EndOfStream))
    }
}
