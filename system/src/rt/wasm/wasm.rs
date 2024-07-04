//! WebAssembly Binary Loader

use super::*;
use alloc::boxed::Box;
use wami::*;

mod maystorm;

pub struct WasmBinaryLoader {
    loaders: Box<[Box<dyn WasmMiniLoader>]>,
}

impl WasmBinaryLoader {
    pub fn new() -> Box<Self> {
        let mut vec = Vec::new();

        vec.push(maystorm::MyosLoader::new());

        Box::new(Self {
            loaders: vec.into_boxed_slice(),
        })
    }
}

impl BinaryLoader for WasmBinaryLoader {
    fn preferred_extension<'a>(&self) -> &'a str {
        "wasm"
    }

    fn recognize(&self, blob: &[u8]) -> bool {
        WebAssembly::identify(blob)
    }

    fn spawn(&self, blob: &[u8], lio: LoadedImageOption) -> Result<ProcessId, megstd::io::Error> {
        let module = WebAssembly::compile(blob).map_err(|v| {
            println!("Compile error: {:?}", v);
            ErrorKind::ExecFormatError
        })?;

        for loader in self.loaders.iter() {
            if loader.recognize(&module) {
                match loader.instantiate(module, lio) {
                    Ok(v) => return Ok(v),
                    Err(err) => {
                        println!("Link error: {:?}", &err);
                        return Err(ErrorKind::Other.into());
                    }
                }
            }
        }

        Err(ErrorKind::Unsupported.into())
    }
}

pub trait WasmMiniLoader {
    fn recognize(&self, module: &WasmModule) -> bool;

    fn instantiate(
        &self,
        module: WasmModule,
        lio: LoadedImageOption,
    ) -> Result<ProcessId, Box<dyn core::error::Error>>;
}
