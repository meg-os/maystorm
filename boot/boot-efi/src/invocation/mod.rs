use crate::page::VirtualAddress;
use bootprot::BootInfo;

cfg_match! {
    cfg(target_arch = "x86_64") => {
        mod amd64;
        pub use amd64::*;
    }
    cfg(target_arch = "x86") => {
        mod x86_32;
        pub use x86_32::*;
    }
}

pub trait Invoke {
    const INCOMPATIBILITY_MESSAGE: &str;

    fn is_compatible(&self) -> bool;

    unsafe fn invoke_kernel(
        &self,
        info: BootInfo,
        entry: VirtualAddress,
        new_sp: VirtualAddress,
    ) -> !;
}
