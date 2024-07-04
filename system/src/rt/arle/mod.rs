//! Arlequin Subsystem (expr)

use super::*;

/// Load .bin file
pub struct ArleBinaryLoader;

impl ArleBinaryLoader {
    #[inline]
    pub fn new() -> Box<Self> {
        Box::new(Self {})
    }

    pub fn identity(blob: &[u8]) -> bool {
        blob[0] == 0xC3
    }

    pub fn start(_: usize) {
        let ctx = Scheduler::current_personality()
            .unwrap()
            .get::<ArleContext>()
            .unwrap();
        unsafe {
            Hal::cpu().invoke_user(ctx.option.start, ctx.option.stack_pointer);
        }
    }
}

impl BinaryLoader for ArleBinaryLoader {
    fn preferred_extension<'a>(&self) -> &'a str {
        "bin"
    }

    fn recognize(&self, blob: &[u8]) -> bool {
        ArleBinaryLoader::identity(blob)
    }

    fn spawn(&self, _blob: &[u8], _lio: LoadedImageOption) -> Result<ProcessId, Error> {
        // SpawnOption::new()
        //     .personality(ArleContext::new(self.option))
        //     .start_process(Self::start, 0, self.lio.name.as_str())
        Err(ErrorKind::Other.into())
    }
}

#[derive(Default)]
pub struct LaunchOption {
    start: usize,
    stack_pointer: usize,
}

pub struct ArleContext {
    option: LaunchOption,
}

impl ArleContext {
    pub fn new(option: LaunchOption) -> PersonalityContext {
        PersonalityContext::new(Self { option })
    }
}

unsafe impl Identify for ArleContext {
    #[rustfmt::skip]
    /// 9CC78BE3-57D3-4E66-BF7E-61B18E8C3C65
    const UUID: Uuid = Uuid::from_parts(0x9CC78BE3, 0x57D3, 0x4E66, 0xBF7E, [0x61, 0xB1, 0x8E, 0x8C, 0x3C, 0x65]);
}

impl Personality for ArleContext {
    fn context(&mut self) -> *mut c_void {
        self as *const _ as *mut c_void
    }

    fn on_exit(self: Box<Self>) {
        // self.windows.lock().unwrap().clear();
    }
}
