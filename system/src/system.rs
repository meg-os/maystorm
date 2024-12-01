//! MEG-OS Kernel
// (c) 2020 Nerry
// License: MIT

use crate::arch::cpu::*;
use crate::io::{screen::*, tty::*};
use crate::task::scheduler::*;
use crate::*;
use bootprot::{BootFlags, BootInfo};
use core::cell::UnsafeCell;
use core::ffi::c_void;
use core::fmt;
use core::mem::{transmute, MaybeUninit};
use core::ptr::{addr_of, addr_of_mut};
use core::sync::atomic::*;
use megstd::drawing::*;
use megstd::time::SystemTime;

#[allow(dead_code)]
pub struct System {
    /// Current device information
    current_device: DeviceInfo,

    /// Array of activated processor cores
    cpus: Vec<Box<Cpu>>,

    /// An instance of ACPI tables
    acpi: Option<&'static myacpi::RsdPtr>,

    /// An instance of SMBIOS
    smbios: Option<Box<fw::smbios::SmBios>>,

    // screens
    safe_screen: MaybeUninit<Option<Arc<BitmapScreen<'static>>>>,
    stdout: Option<Box<dyn Tty>>,
    emcon: MaybeUninit<Box<UnsafeCell<io::emcon::EmConsole>>>,

    // copy of boot info
    boot_flags: BootFlags,
    initrd_base: PhysicalAddress,
    initrd_size: usize,
}

static mut SYSTEM: UnsafeCell<System> = UnsafeCell::new(System::new());

impl System {
    const SYSTEM_NAME: &'static str = "MEG-OS";
    const SYSTEM_CODENAME: &'static str = "codename Maystorm-13";
    const SYSTEM_SHORT_NAME: &'static str = "myos";
    const RELEASE: &'static str = "alpha";
    const VERSION: Version<'static> = Version::new(0, 13, 0, Self::RELEASE);

    #[inline]
    const fn new() -> Self {
        System {
            current_device: DeviceInfo::new(),
            cpus: Vec::new(),
            acpi: None,
            smbios: None,
            boot_flags: BootFlags::empty(),
            safe_screen: MaybeUninit::zeroed(),
            emcon: MaybeUninit::zeroed(),
            stdout: None,
            initrd_base: PhysicalAddress::NULL,
            initrd_size: 0,
        }
    }

    /// Initialize the system
    pub unsafe fn init(info: &BootInfo, main: fn() -> ()) -> ! {
        assert_call_once!();

        let shared = (&mut *addr_of_mut!(SYSTEM)).get_mut();
        shared.boot_flags = info.flags;
        shared.initrd_base = PhysicalAddress::new(info.initrd_base as u64);
        shared.initrd_size = info.initrd_size as usize;
        shared.current_device.total_memory_size = info.total_memory_size as usize;

        mem::MemoryManager::init_first(info);

        if info.vram_base > 0
            && info.vram_stride > 0
            && info.screen_width > 0
            && info.screen_height > 0
        {
            let stride = info.vram_stride as usize;
            let vram_size = 4 * stride * info.screen_height as usize;
            let base = mem::MemoryManager::mmap(mem::MemoryMapRequest::Framebuffer(
                PhysicalAddress::new(info.vram_base),
                vram_size,
            ))
            .unwrap()
            .get() as *mut TrueColor;
            let size = Size::new(info.screen_width as u32, info.screen_height as u32);
            let screen = BitmapScreen::new(BitmapRefMut32::from_static(base, size, stride));
            screen
                .set_orientation(ScreenOrientation::Landscape)
                .unwrap();
            shared.safe_screen.write(Some(Arc::new(screen)));

            shared
                .emcon
                .write(Box::new(UnsafeCell::new(io::emcon::EmConsole::new(
                    ui::font::FontManager::fixed_system_font(),
                ))));
        }

        shared.acpi = unsafe { myacpi::RsdPtr::parse(info.acpi_rsdptr as usize as *const c_void) };

        if info.smbios != 0 {
            let device = &mut shared.current_device;
            let smbios = fw::smbios::SmBios::init(info.smbios.into());
            device.manufacturer_name = smbios.manufacturer_name();
            device.model_name = smbios.model_name();
            shared.smbios = Some(smbios);
        }

        arch::Arch::init_first(info);

        Scheduler::start(Self::_init_second, main as usize);
    }

    /// The second half of the system initialization
    fn _init_second(args: usize) {
        assert_call_once!();

        let shared = Self::shared();

        unsafe {
            utils::EventManager::init();
            Scheduler::init_second();
            mem::MemoryManager::init_second();
            fs::FileManager::init(shared.initrd_base.direct_map(), shared.initrd_size);

            io::hid_mgr::HidManager::init();
            io::audio::AudioManager::init();
            drivers::usb::UsbManager::init();

            drivers::pci::Pci::init();
            arch::Arch::init_second();

            ui::font::FontManager::init();
            if let Some(main_screen) = Self::main_screen() {
                ui::window::WindowManager::init(main_screen);
            }

            rt::RuntimeEnvironment::init();

            init::SysInit::start(transmute(args));
        }
    }

    #[inline]
    unsafe fn shared_mut() -> &'static mut System {
        (&mut *addr_of_mut!(SYSTEM)).get_mut()
    }

    #[inline]
    fn shared() -> &'static System {
        unsafe { &*(&*addr_of!(SYSTEM)).get() }
    }

    /// Returns the name of the current system.
    #[inline]
    pub const fn name() -> &'static str {
        &Self::SYSTEM_NAME
    }

    /// Returns the codename of the current system.
    #[inline]
    pub const fn codename() -> &'static str {
        &Self::SYSTEM_CODENAME
    }

    /// Returns abbreviated name of the current system.
    #[inline]
    pub const fn short_name() -> &'static str {
        &Self::SYSTEM_SHORT_NAME
    }

    /// Returns the version of the current system.
    #[inline]
    pub const fn version<'a>() -> &'a Version<'a> {
        &Self::VERSION
    }

    #[inline]
    pub fn boot_flags() -> BootFlags {
        Self::shared().boot_flags
    }

    /// Returns the current system time.
    #[inline]
    pub fn system_time() -> SystemTime {
        arch::Arch::system_time()
    }

    /// Returns whether the kernel is multiprocessor-capable.
    #[inline]
    pub const fn is_multi_processor_capable_kernel() -> bool {
        Scheduler::is_multi_processor_capable()
    }

    /// Add SMP-initialized CPU cores to the list of activated cores.
    ///
    /// # Safety
    ///
    /// THREAD UNSAFE.
    /// Do not call this function except when initializing the SMP.
    #[inline]
    pub unsafe fn activate_cpu(new_cpu: Box<Cpu>) {
        let shared = Self::shared_mut();
        let processor_type = new_cpu.processor_type();
        shared.cpus.push(new_cpu);
        let device = &shared.current_device;
        device.num_of_logical_cpus.fetch_add(1, Ordering::AcqRel);
        match processor_type {
            ProcessorCoreType::Normal => {
                device.num_of_main_cpus.fetch_add(1, Ordering::AcqRel);
                device.num_of_physical_cpus.fetch_add(1, Ordering::AcqRel);
            }
            ProcessorCoreType::Efficient => {
                device.num_of_effecient_cpus.fetch_add(1, Ordering::AcqRel);
                device.num_of_physical_cpus.fetch_add(1, Ordering::AcqRel);
                if device.num_of_main_cpus() > 0 {
                    device.is_hybrid.store(true, Ordering::SeqCst);
                }
            }
            ProcessorCoreType::Sub | ProcessorCoreType::EfficientSub => {
                device.has_smt.store(true, Ordering::SeqCst);
            }
        }

        fence(Ordering::SeqCst);
    }

    #[inline]
    pub fn cpus<'a>() -> impl ExactSizeIterator<Item = &'a Box<Cpu>> {
        Self::shared().cpus.iter()
    }

    /// Returns a reference to the processor at the specified index.
    ///
    /// # Panics
    ///
    /// Panics if specified index is larger than the number of processors.
    #[inline]
    #[track_caller]
    pub fn cpu<'a>(index: ProcessorIndex) -> &'a Cpu {
        Self::cpu_ref(index).unwrap()
    }

    #[inline]
    pub fn cpu_ref<'a>(index: ProcessorIndex) -> Option<&'a Box<Cpu>> {
        Self::shared().cpus.get(index.0)
    }

    #[inline]
    pub fn smbios<'a>() -> Option<&'a fw::smbios::SmBios> {
        Self::shared().smbios.as_ref().map(|v| v.as_ref())
    }

    /// Returns the current device information.
    #[inline]
    pub fn current_device<'a>() -> &'a DeviceInfo {
        &Self::shared().current_device
    }

    #[inline]
    pub fn acpi<'a>() -> Option<&'a myacpi::Xsdt> {
        Self::shared().acpi.as_ref().map(|v| v.xsdt())
    }

    #[inline]
    pub fn safe_screen<'a>() -> Option<Arc<BitmapScreen<'static>>> {
        unsafe {
            Self::shared()
                .safe_screen
                .assume_init_ref()
                .as_ref()
                .map(|v| v.clone())
        }
    }

    /// Get main screen
    #[inline]
    pub fn main_screen() -> Option<Arc<dyn Screen<BitmapRef32<'static>, ColorType = TrueColor>>> {
        Self::safe_screen()
            .map(|v| v as Arc<dyn Screen<BitmapRef32<'static>, ColorType = TrueColor>>)
    }

    pub fn set_stdout(stdout: Box<dyn Tty>) {
        let shared = unsafe { Self::shared_mut() };
        shared.stdout = Some(stdout);
    }

    pub fn stdout<'a>() -> &'a mut dyn Tty {
        let shared = unsafe { Self::shared_mut() };
        shared
            .stdout
            .as_mut()
            .map(|v| v.as_mut())
            .unwrap_or(io::tty::NullTty::null())
    }

    pub fn log<'a>() -> &'a mut dyn Tty {
        // Self::stdout()
        unsafe {
            let shared = Self::shared_mut();
            shared.emcon.assume_init_mut().get_mut()
        }
    }

    #[track_caller]
    pub fn assert_call_once(mutex: &'static AtomicBool) {
        if mutex
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::Relaxed)
            .is_err()
        {
            panic!("Multiple calls are not allowed");
        }
    }

    pub fn date_to_integer(y: u16, m: u8, d: u8) -> u32 {
        if y < 1970 {
            return 0;
        }
        let m = match Month::try_from(m) {
            Some(v) => v,
            None => return 0,
        };

        let mut acc = (y as u32 * 365) + ((y / 4) - (y / 100) + (y / 400)) as u32;

        let md_tbl = [0, 31, 59, 90, 120, 151, 181, 212, 243, 273, 304, 334];
        acc += md_tbl[m as usize];
        if m > Month::Feb && ((y % 400) == 0 || (y % 4) == 0 && (y % 100) != 0) {
            acc += 1;
        }

        acc += d as u32;

        acc - 719528 - 1
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Month {
    Jan,
    Feb,
    Mar,
    Apr,
    May,
    Jun,
    Jul,
    Aug,
    Sep,
    Oct,
    Nov,
    Dec,
}

impl Month {
    #[inline]
    fn try_from(v: u8) -> Option<Self> {
        match v {
            1 => Some(Self::Jan),
            2 => Some(Self::Feb),
            3 => Some(Self::Mar),
            4 => Some(Self::Apr),
            5 => Some(Self::May),
            6 => Some(Self::Jun),
            7 => Some(Self::Jul),
            8 => Some(Self::Aug),
            9 => Some(Self::Sep),
            10 => Some(Self::Oct),
            11 => Some(Self::Nov),
            12 => Some(Self::Dec),
            _ => None,
        }
    }
}

#[macro_export]
macro_rules! assert_call_once {
    () => {
        static MUTEX: core::sync::atomic::AtomicBool = core::sync::atomic::AtomicBool::new(false);
        crate::system::System::assert_call_once(&MUTEX);
    };
}

static PANIC_GLOBAL_LOCK: Spinlock = Spinlock::new();

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    unsafe {
        Hal::cpu().disable_interrupt();
        task::scheduler::Scheduler::freeze(true);
        PANIC_GLOBAL_LOCK.synchronized(|| {
            let stdout = System::log();
            stdout.set_attribute(0x4F);
            if let Some(thread) = task::scheduler::Scheduler::current_thread() {
                if let Some(name) = thread.name() {
                    let _ = write!(stdout, "thread '{}' ", name);
                } else {
                    let _ = write!(stdout, "thread {} ", thread.as_usize());
                }
            }
            let _ = writeln!(stdout, "{}", info);
        });
        Hal::cpu().stop();
    }
}

pub struct DeviceInfo {
    manufacturer_name: Option<String>,
    model_name: Option<String>,
    num_of_logical_cpus: AtomicUsize,
    num_of_physical_cpus: AtomicUsize,
    num_of_main_cpus: AtomicUsize,
    num_of_effecient_cpus: AtomicUsize,
    is_hybrid: AtomicBool,
    has_smt: AtomicBool,
    total_memory_size: usize,
}

impl DeviceInfo {
    #[inline]
    const fn new() -> Self {
        Self {
            manufacturer_name: None,
            model_name: None,
            num_of_logical_cpus: AtomicUsize::new(0),
            num_of_physical_cpus: AtomicUsize::new(0),
            num_of_main_cpus: AtomicUsize::new(0),
            num_of_effecient_cpus: AtomicUsize::new(0),
            is_hybrid: AtomicBool::new(false),
            has_smt: AtomicBool::new(false),
            total_memory_size: 0,
        }
    }

    /// Returns the name of the manufacturer of the system, if available.
    #[inline]
    pub fn manufacturer_name(&self) -> Option<&str> {
        self.manufacturer_name.as_ref().map(|v| v.as_str())
    }

    /// Returns the model name of the system, if available.
    #[inline]
    pub fn model_name(&self) -> Option<&str> {
        self.model_name.as_ref().map(|v| v.as_str())
    }

    /// Returns the total amount of memory size in bytes.
    #[inline]
    pub const fn total_memory_size(&self) -> usize {
        self.total_memory_size
    }

    /// Returns the number of active logical CPU cores.
    #[inline]
    pub fn num_of_logical_cpus(&self) -> usize {
        self.num_of_logical_cpus.load(Ordering::SeqCst)
    }

    /// Returns the number of physical CPU cores.
    /// Returns less than `num_of_logical_cpus` for SMT-enabled processors.
    #[inline]
    pub fn num_of_physical_cpus(&self) -> usize {
        self.num_of_physical_cpus.load(Ordering::SeqCst)
    }

    /// Returns the number of performance CPU cores.
    #[inline]
    pub fn num_of_main_cpus(&self) -> usize {
        self.num_of_main_cpus.load(Ordering::SeqCst)
    }

    /// Returns the number of Highly efficient CPU cores.
    #[inline]
    pub fn num_of_efficient_cpus(&self) -> usize {
        self.num_of_effecient_cpus.load(Ordering::SeqCst)
    }

    #[inline]
    pub fn processor_system_type(&self) -> ProcessorSystemType {
        if self.is_hybrid.load(Ordering::Relaxed) {
            ProcessorSystemType::Hybrid
        } else if self.has_smt.load(Ordering::Relaxed) {
            ProcessorSystemType::SMT
        } else if self.num_of_logical_cpus() > 1 {
            ProcessorSystemType::SMP
        } else {
            ProcessorSystemType::Uniprocessor
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum ProcessorSystemType {
    /// System is a hybrid of performance and high-efficiency cores
    Hybrid,
    SMT,
    SMP,
    Uniprocessor,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Version<'a> {
    versions: u32,
    rel: &'a str,
}

impl Version<'_> {
    #[inline]
    pub const fn new<'a>(maj: u8, min: u8, patch: u16, rel: &'a str) -> Version<'a> {
        let versions = ((maj as u32) << 24) | ((min as u32) << 16) | (patch as u32);
        Version { versions, rel }
    }

    #[inline]
    pub const fn as_u32(&self) -> u32 {
        self.versions
    }

    #[inline]
    pub const fn maj(&self) -> usize {
        ((self.versions >> 24) & 0xFF) as usize
    }

    #[inline]
    pub const fn min(&self) -> usize {
        ((self.versions >> 16) & 0xFF) as usize
    }

    #[inline]
    pub const fn patch(&self) -> usize {
        (self.versions & 0xFFFF) as usize
    }

    #[inline]
    pub const fn rel(&self) -> &str {
        &self.rel
    }
}

impl fmt::Display for Version<'_> {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.rel().len() > 0 {
            write!(
                f,
                "{}.{}.{}-{}",
                self.maj(),
                self.min(),
                self.patch(),
                self.rel(),
            )
        } else {
            write!(f, "{}.{}.{}", self.maj(), self.min(), self.patch())
        }
    }
}

#[repr(transparent)]
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct ProcessorIndex(pub usize);

impl ProcessorIndex {
    #[inline]
    pub fn get<'a>(&self) -> Option<&'a Box<Cpu>> {
        System::cpu_ref(*self)
    }
}

impl From<ProcessorIndex> for usize {
    #[inline]
    fn from(value: ProcessorIndex) -> Self {
        value.0
    }
}

impl From<usize> for ProcessorIndex {
    #[inline]
    fn from(value: usize) -> Self {
        Self(value)
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum ProcessorCoreType {
    /// Normal Processor
    Normal,
    /// Subprocessor of SMT enabled processor.
    Sub,
    /// Highly Efficient Processor
    Efficient,
    /// Highly Efficient Subprocessor
    EfficientSub,
}

impl ProcessorCoreType {
    #[inline]
    pub fn new(is_normal: bool, is_efficient: bool) -> Self {
        match (is_normal, is_efficient) {
            (true, true) => Self::Efficient,
            (true, false) => Self::Normal,
            (false, true) => Self::EfficientSub,
            (false, false) => Self::Sub,
        }
    }

    #[inline]
    pub const fn is_normal_processor(&self) -> bool {
        match *self {
            Self::Normal | Self::Efficient => true,
            Self::Sub | Self::EfficientSub => false,
        }
    }

    #[inline]
    pub const fn is_sub_processor(&self) -> bool {
        !self.is_normal_processor()
    }

    #[inline]
    pub const fn is_performance_processor(&self) -> bool {
        !self.is_efficient_processor()
    }

    #[inline]
    pub const fn is_efficient_processor(&self) -> bool {
        match *self {
            Self::Efficient | Self::EfficientSub => true,
            Self::Normal | Self::Sub => false,
        }
    }
}
