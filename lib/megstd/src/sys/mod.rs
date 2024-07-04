pub mod megos;

cfg_match! {
    cfg(test) => {
        mod tests;
        pub use tests::*;
    }
    cfg(feature = "wasm") => {
        #[macro_use]
        mod wasm;
        pub use wasm::*;
    }
    cfg(feature = "kernel") => {
        #[macro_use]
        mod kernel;
        pub use kernel::*;
    }
}
