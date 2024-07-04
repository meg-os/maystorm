//! Runtime Environment and Personalities

use crate::fs::*;
use crate::task::scheduler::*;
use crate::*;
use core::cell::UnsafeCell;
use core::ffi::c_void;
use megstd::io::{Error, ErrorKind, Read};
use megstd::path::Path;
use megstd::uuid::{Identify, Uuid};

pub mod arle;

#[path = "wasm/wasm.rs"]
pub mod wasm;

#[cfg(target_arch = "x86_64")]
#[path = "haribote/hoe.rs"]
pub mod haribote;

static mut RE: UnsafeCell<RuntimeEnvironment> = UnsafeCell::new(RuntimeEnvironment::new());

pub struct RuntimeEnvironment {
    path_ext: Vec<String>,
    image_loaders: Vec<Box<dyn BinaryLoader>>,
}

impl RuntimeEnvironment {
    #[inline]
    const fn new() -> Self {
        Self {
            path_ext: Vec::new(),
            image_loaders: Vec::new(),
        }
    }

    #[inline]
    pub unsafe fn init() {
        assert_call_once!();

        let shared = &mut *RE.get();

        shared.add_image(arle::ArleBinaryLoader::new());

        shared.add_image(wasm::WasmBinaryLoader::new());

        #[cfg(target_arch = "x86_64")]
        shared.add_image(haribote::HrbBinaryLoader::new());
    }

    #[inline]
    fn add_image(&mut self, loader: Box<dyn BinaryLoader>) {
        self.path_ext.push(loader.preferred_extension().to_string());
        self.image_loaders.push(loader);
    }

    #[inline]
    fn shared<'a>() -> &'a Self {
        unsafe { &*RE.get() }
    }

    #[inline]
    pub fn supported_extensions<'a>() -> impl Iterator<Item = &'a String> {
        Self::shared().path_ext.iter()
    }

    pub fn spawn(path: &str, args: &[&str]) -> Result<ProcessId, Error> {
        let mut fcb = FileManager::open(path, OpenOptions::new().read(true))?;
        let stat = fcb.fstat().unwrap();
        if !stat.file_type().is_file() {
            return Err(ErrorKind::PermissionDenied.into());
        }
        let file_size = stat.len() as usize;
        if file_size > 0 {
            let mut vec = Vec::with_capacity(file_size);
            fcb.read_to_end(&mut vec)?;
            let blob = vec.as_slice();
            let shared = Self::shared();
            for loader in &shared.image_loaders {
                if loader.recognize(blob) {
                    let lpc = Path::new(path)
                        .file_name()
                        .and_then(|v| v.to_str())
                        .unwrap_or_default();
                    return loader.spawn(blob, LoadedImageOption::new(lpc, args));
                }
            }
            return Err(ErrorKind::ExecFormatError.into());
        } else {
            return Err(ErrorKind::ExecFormatError.into());
        }
    }

    #[inline]
    pub fn exit(_exit_code: usize) -> ! {
        Scheduler::exit();
    }
}

/// Contains a reference to the context of the current personality
pub struct PersonalityContext {
    uuid: Uuid,
    payload: Box<dyn Personality>,
}

pub trait Personality {
    /// Returns its own context
    fn context(&mut self) -> *mut c_void;

    /// Called to clean up resources before the process ends.
    fn on_exit(self: Box<Self>);
}

impl PersonalityContext {
    #[inline]
    pub fn new<T: Personality + Identify + 'static>(payload: T) -> Self {
        Self {
            uuid: T::UUID,
            payload: Box::new(payload),
        }
    }

    #[inline]
    pub fn get<'a, T: Identify>(&'a mut self) -> Result<&'a mut T, Uuid> {
        (T::UUID == self.uuid())
            .then(|| unsafe { &mut *(self.payload.context() as *mut T) })
            .ok_or(self.uuid())
    }

    #[inline]
    pub const fn uuid(&self) -> Uuid {
        self.uuid
    }

    #[inline]
    pub fn on_exit(self) {
        self.payload.on_exit();
    }
}

pub trait BinaryLoader {
    fn preferred_extension<'a>(&self) -> &'a str;

    fn recognize(&self, blob: &[u8]) -> bool;

    fn spawn(&self, blob: &[u8], lio: LoadedImageOption) -> Result<ProcessId, Error>;
}

#[derive(Debug, Default)]
pub struct LoadedImageOption {
    pub name: String,
    pub argv: Vec<String>,
}

impl LoadedImageOption {
    #[inline]
    pub fn new(name: &str, args: &[&str]) -> Self {
        Self {
            name: name.to_string(),
            argv: args.iter().map(|v| v.to_string()).collect(),
        }
    }
}

/// Contextual data for legacy applications
#[derive(Debug, Default, Copy, Clone)]
pub struct LegacyAppContext {
    /// Base address of the application image
    pub image_base: u32,
    /// Size of the application image
    pub image_size: u32,
    /// Base address of the code segment
    pub base_of_code: u32,
    /// Size of the code segment
    pub size_of_code: u32,
    /// Base address of the data segment
    pub base_of_data: u32,
    /// Size of the data segment
    pub size_of_data: u32,
    /// Application entry point
    pub start: u32,
    /// Initial stack pointer
    pub stack_pointer: u32,
}
