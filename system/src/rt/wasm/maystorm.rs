//! MEG-OS Maystorm2020 Subsystem
use super::*;
use crate::io::hid_mgr::*;
use crate::sync::Mutex;
use crate::system::System;
use crate::ui::text::*;
use crate::ui::theme::Theme;
use crate::ui::window::*;
use core::alloc::Layout;
use core::intrinsics::transmute;
use core::num::NonZeroU32;
use core::sync::atomic::*;
use core::time::Duration;
use megstd::drawing::*;
use megstd::io::Write;
use megstd::rand::*;
use megstd::time::SystemTime;
use megstd::uuid::identify;
use wami::prelude::*;

pub struct MyosLoader;

impl MyosLoader {
    #[inline]
    pub fn new() -> Box<dyn WasmMiniLoader> {
        Box::new(Self {})
    }

    fn start(_: usize) {
        Scheduler::current_personality()
            .unwrap()
            .get::<MyosRuntime>()
            .unwrap()
            .start();
    }
}

impl WasmMiniLoader for MyosLoader {
    fn recognize(&self, module: &WasmModule) -> bool {
        module
            .imports()
            .find(|item| {
                item.kind == ImportExportKind::Function && item.module == MyosRuntime::MOD_NAME
            })
            .and_then(|_| {
                module.exports().find(|item| {
                    item.kind == ImportExportKind::Function
                        && item.name == MyosRuntime::ENTRY_FUNC_NAME
                })
            })
            .is_some()
    }

    fn instantiate(
        &self,
        module: WasmModule,
        lio: LoadedImageOption,
    ) -> Result<ProcessId, Box<dyn core::error::Error>> {
        let instance = module.instantiate(self)?;

        SpawnOption::new()
            .personality(MyosRuntime::new(instance))
            .start_process(Self::start, 0, lio.name.as_ref())
            .map_err(|err| Box::new(err) as Box<dyn core::error::Error>)
    }
}

impl WasmEnv for MyosLoader {
    fn resolve_import_func(
        &self,
        mod_name: &str,
        name: &str,
        type_: &WasmType,
    ) -> WasmImportFuncResult {
        let signature = type_.signature();
        match mod_name {
            MyosRuntime::MOD_NAME => match (name, signature.as_str()) {
                ("svc0", "ii") => WasmImportFuncResult::Ok(MyosRuntime::syscall),
                ("svc1", "iii") => WasmImportFuncResult::Ok(MyosRuntime::syscall),
                ("svc2", "iiii") => WasmImportFuncResult::Ok(MyosRuntime::syscall),
                ("svc3", "iiiii") => WasmImportFuncResult::Ok(MyosRuntime::syscall),
                ("svc4", "iiiiii") => WasmImportFuncResult::Ok(MyosRuntime::syscall),
                ("svc5", "iiiiiii") => WasmImportFuncResult::Ok(MyosRuntime::syscall),
                ("svc6", "iiiiiiii") => WasmImportFuncResult::Ok(MyosRuntime::syscall),
                _ => WasmImportFuncResult::NoMethod,
            },
            _ => WasmImportFuncResult::NoModule,
        }
    }
}

#[wasm_exports]
trait MyosExports {
    fn _start();
}

#[allow(dead_code)]
#[identify("57392D77-D199-486E-9A2C-47D15BA6DFCA")]
pub struct MyosRuntime {
    instance: WasmInstance,
    next_handle: AtomicUsize,
    windows: Mutex<BTreeMap<usize, UnsafeCell<OsWindow>>>,
    files: Mutex<Vec<Option<Arc<Mutex<FsRawFileControlBlock>>>>>,
    rng32: XorShift32,
    key_buffer: Mutex<Vec<KeyEvent>>,
    malloc: Mutex<SimpleAllocator>,
    has_to_exit: AtomicBool,
    throttle_timer_expired: AtomicBool,
    fps_throttle: Mutex<Option<ThrottleState>>,
}

impl Personality for MyosRuntime {
    fn context(&mut self) -> *mut c_void {
        self as *const _ as *mut c_void
    }

    fn on_exit(self: Box<Self>) {
        self.windows.lock().unwrap().clear();
    }
}

impl MyosRuntime {
    const MAX_FILES: usize = 20;
    const MOD_NAME: &'static str = "megos-canary";
    const ENTRY_FUNC_NAME: &'static str = "_start";

    const SIZE_KEYBUFFER: usize = 32;

    fn new(instance: WasmInstance) -> PersonalityContext {
        PersonalityContext::new(Self {
            instance,
            next_handle: AtomicUsize::new(1),
            windows: Mutex::new(BTreeMap::new()),
            files: Mutex::new(Vec::new()),
            rng32: XorShift32::default(),
            key_buffer: Mutex::new(Vec::with_capacity(Self::SIZE_KEYBUFFER)),
            malloc: Mutex::new(SimpleAllocator::default()),
            has_to_exit: AtomicBool::new(false),
            throttle_timer_expired: AtomicBool::new(false),
            fps_throttle: Mutex::new(None),
        })
    }

    fn next_handle(&self) -> usize {
        let result = 1 + self.next_handle.load(Ordering::SeqCst);
        self.next_handle.swap(result, Ordering::SeqCst)
    }

    fn start(&self) -> ! {
        match self.instance.exports()._start() {
            Ok(_) => (),
            Err(err) => match err.downcast_ref::<WasmRuntimeError>() {
                Some(err) => match err.kind() {
                    WasmRuntimeErrorKind::Exit => (),
                    _ => println!("error: {:?}", err),
                },
                None => {
                    println!("error: {:?}", err)
                }
            },
        }

        RuntimeEnvironment::exit(0);
    }

    fn syscall(_: &WasmInstance, args: WasmArgs) -> WasmDynResult {
        Scheduler::current_personality()
            .unwrap()
            .get::<Self>()
            .unwrap()
            .dispatch_syscall(args)
            .map(|v| Some(v.into()))
            .map_err(|e| e.into())
    }

    fn dispatch_syscall(&mut self, args: WasmArgs) -> Result<i32, WasmRuntimeErrorKind> {
        use megstd::sys::megos::svc::Function;

        let mut params = ParamsDecoder::new(args);
        let memory = self
            .instance
            .memory(0)
            .ok_or(WasmRuntimeErrorKind::OutOfMemory)?;
        let func_no = params
            .get_u32()
            .map(|v| unsafe { transmute::<u32, Function>(v) })?;

        if self.has_to_exit.load(Ordering::Relaxed) {
            return Err(WasmRuntimeErrorKind::Exit);
        }

        match func_no {
            Function::Exit => {
                return Err(WasmRuntimeErrorKind::Exit);
            }

            Function::Monotonic => return Ok(Timer::monotonic().as_micros() as i32),
            Function::Time => {
                let sub_func_no = params.get_usize()?;
                match sub_func_no {
                    0 => {
                        let memory = memory.try_borrow()?;
                        let offset = params.get_u32()?;
                        let result: &mut SystemTime =
                            unsafe { memory.transmute_mut(WasmPtrMut::from_u32(offset)) }?;
                        *result = System::system_time();
                        return Ok(0);
                    }
                    1 => {
                        let memory = memory.try_borrow()?;
                        let offset = params.get_u32()?;
                        let result: &mut Duration =
                            unsafe { memory.transmute_mut(WasmPtrMut::from_u32(offset)) }?;
                        *result = Timer::monotonic();
                        return Ok(0);
                    }
                    _ => (),
                }
            }
            Function::Usleep => {
                let us = params.get_u32()? as u64;
                Timer::sleep(Duration::from_micros(us));
            }

            Function::GetSystemInfo => {
                let sub_func_no = params.get_usize()?;
                match sub_func_no {
                    0 => return Ok(System::version().as_u32() as i32),
                    _ => (),
                }
            }

            Function::PrintString => {
                params.get_string(memory).map(|s| print!("{}", s));
            }

            Function::Open => {
                let path = params
                    .get_string(memory)
                    .ok_or(WasmRuntimeErrorKind::InvalidParameter)?;
                let _options = params.get_u32()?;
                return Self::encode_io_result(
                    FileManager::open(path, OpenOptions::new().read(true))
                        .and_then(|file| self.alloc_file(file)),
                );
            }
            Function::Close => {
                let handle = params.get_usize()?;
                self.close_file(handle);
            }
            Function::Read => {
                let file = params.get_file(self)?;
                let buf = params.get_buffer(memory)?;
                return Self::encode_io_result(file.lock().unwrap().read(buf));
            }
            Function::Write => {
                let file = params.get_file(self)?;
                let buf = params.get_buffer(memory)?;
                return Self::encode_io_result(file.lock().unwrap().write(buf));
            }
            Function::LSeek => {
                let file = params.get_file(self)?;
                let offset = params.get_i32()? as OffsetType;
                let whence = Whence::try_from(params.get_usize()?)
                    .map_err(|_| WasmRuntimeErrorKind::InvalidParameter)?;
                return Self::encode_io_result(
                    file.lock()
                        .unwrap()
                        .lseek(offset, whence)
                        .map(|v| v as usize),
                );
            }

            Function::NewWindow => {
                let title = params.get_string(memory).unwrap_or("");
                let size = params.get_size()?;
                let bg_color = params
                    .get_color()
                    .unwrap_or(Theme::shared().window_default_background());
                let window_option = params.get_u32().unwrap_or_default();

                let window = RawWindowBuilder::new()
                    .with_options(window_option)
                    .size(size)
                    .bg_color(bg_color)
                    .build(title);

                if window.as_usize() != 0 {
                    let handle = self.next_handle();
                    let window = UnsafeCell::new(OsWindow::new(handle, window));
                    self.windows.lock().unwrap().insert(handle, window);
                    return Ok(handle as i32);
                }
            }
            Function::CloseWindow => {
                let handle = params.get_usize()?;
                self.windows.lock().unwrap().remove(&handle);
            }
            Function::BeginDraw => match params.get_window(self) {
                Ok(window) => {
                    window.begin_draw();
                    return Ok(window.handle() as i32);
                }
                Err(err) => return Err(err),
            },
            Function::EndDraw => match params.get_window(self) {
                Ok(window) => {
                    window.end_draw();
                    self.wait_throttle(window.native())?;
                }
                Err(err) => return Err(err),
            },

            Function::DrawString => {
                let window = params.get_window(self)?;
                let origin = params.get_point()?;
                let text = params.get_string(memory).unwrap_or("");
                let color = params.get_color()?;
                let mut rect = window.content_rect().bounds();
                rect.origin = origin;
                rect.size.width -= origin.x as u32;
                rect.size.height -= origin.y as u32;
                window.draw_in_rect(rect, |bitmap| {
                    AttributedString::new()
                        .align(TextAlignment::Left)
                        .valign(VerticalAlignment::Top)
                        .color(color)
                        .text(text)
                        .draw_text(bitmap, rect.bounds(), 0);
                });
            }
            Function::FillRect => {
                let window = params.get_window(self)?;
                let origin = params.get_point()?;
                let size = params.get_size()?;
                let color = params.get_color()?;
                let rect = Rect { origin, size };
                window.draw_in_rect(rect, |bitmap| {
                    bitmap.fill_rect(rect.bounds(), color);
                });
            }
            Function::DrawRect => {
                let window = params.get_window(self)?;
                let origin = params.get_point()?;
                let size = params.get_size()?;
                let color = params.get_color()?;
                let rect = Rect { origin, size };
                window.draw_in_rect(rect, |bitmap| {
                    bitmap.draw_rect(rect.bounds(), color);
                });
            }
            Function::DrawLine => {
                let window = params.get_window(self)?;
                let c1 = params.get_point()?;
                let c2 = params.get_point()?;
                let color = params.get_color()?;
                let rect = Rect::from(Coordinates::from_diagonal(c1, c2)) + Size::new(1, 1);
                let offset = Point::from(rect.origin());
                window.draw_in_rect(rect, |bitmap| {
                    bitmap.draw_line(c1 - offset, c2 - offset, color);
                });
            }
            Function::DrawShape => {
                let window = params.get_window(self)?;
                let origin = params.get_point()?;
                let size = params.get_size()?;

                let offset = params.get_u32()?;
                const LEN: usize = 3;
                let memory = memory.try_borrow()?;
                let params: &[u32; LEN] =
                    memory
                        .slice(WasmPtr::from_u32(offset), LEN)
                        .and_then(|v: &[u32]| {
                            v.try_into()
                                .map_err(|_| WasmRuntimeErrorKind::InvalidParameter)
                        })?;
                let radius = params[0];
                let bg_color = PackedColor::from_raw(params[1]).as_color();
                let border_color = PackedColor::from_raw(params[2]).as_color();

                let rect = Rect { origin, size };
                window.draw_in_rect(rect, |bitmap| {
                    if bg_color != Color::TRANSPARENT {
                        bitmap.fill_round_rect(bitmap.bounds(), radius, bg_color);
                    }
                    if border_color != Color::TRANSPARENT {
                        bitmap.draw_round_rect(bitmap.bounds(), radius, border_color);
                    }
                });
            }
            Function::WindowFpsThrottle => {
                let _window = params.get_window(self)?;
                let fps = params.get_usize()?;
                if fps > 0 {
                    *self.fps_throttle.lock().unwrap() = Some(ThrottleState::new(fps));
                } else {
                    *self.fps_throttle.lock().unwrap() = None;
                }
            }

            Function::WaitChar => {
                let window = params.get_window(self)?;
                return self
                    .wait_key(window.native())
                    .map(|c| c.unwrap_or('\0') as i32);
            }
            Function::ReadChar => {
                let window = params.get_window(self)?;
                let c = self.read_key(window.native());
                return Ok(c
                    .map(|v| v as i32)
                    .unwrap_or(megstd::sys::megos::OPTION_CHAR_NONE as i32));
            }

            Function::Blt8 => {
                let window = params.get_window(self)?;
                let origin = params.get_point()?;
                let src = params.get_bitmap8(memory)?;
                if let Ok(size) = params.get_size() {
                    let rect = Rect { origin, size };
                    window.draw_in_rect(rect, |bitmap| {
                        bitmap.blt_transparent(
                            &BitmapRef::from(&src),
                            Point::default(),
                            rect,
                            IndexedColor::KEY_COLOR,
                        )
                    })
                } else {
                    let rect = Rect {
                        origin,
                        size: src.size(),
                    };
                    window.draw_in_rect(rect, |bitmap| {
                        bitmap.blt_transparent(
                            &BitmapRef::from(&src),
                            Point::default(),
                            src.size().into(),
                            IndexedColor::KEY_COLOR,
                        );
                    });
                }
            }
            Function::Blt32 => {
                let window = params.get_window(self)?;
                let origin = params.get_point()?;
                let src = params.get_bitmap32(memory)?;
                if let Ok(size) = params.get_size() {
                    let rect = Rect { origin, size };
                    window.draw_in_rect(rect, |bitmap| {
                        bitmap.blt(&BitmapRef::from(&src), Point::default(), rect)
                    })
                } else {
                    let rect = Rect {
                        origin,
                        size: src.size(),
                    };
                    window.draw_in_rect(rect, |bitmap| {
                        bitmap.blt(&BitmapRef::from(&src), Point::default(), src.size().into());
                    });
                }
            }
            Function::BlendRect => {
                let bitmap = params.get_bitmap32(memory)?;
                let origin = params.get_point()?;
                let size = params.get_size()?;
                let color = params.get_u32().map(|v| TrueColor::from_argb(v))?;
                let rect = Rect { origin, size };
                let mut bitmap: BitmapRefMut32 = unsafe { transmute(bitmap) };
                bitmap.blend_rect(rect, color);
            }
            Function::Blt1 => {
                let window = params.get_window(self)?;
                let origin = params.get_point()?;
                let os_bitmap = params.get_bitmap1(memory)?;
                let color = params.get_color()?;
                let mode = params.get_usize()?;
                window.draw_in_rect(os_bitmap.rect(origin, mode), |bitmap| {
                    os_bitmap.blt(bitmap, Point::default(), color, mode);
                });
            }

            Function::Rand => return Ok(self.rng32.next() as i32),
            Function::Srand => {
                let seed = params.get_u32()?;
                NonZeroU32::new(seed).map(|v| self.rng32 = XorShift32::new(v));
            }

            Function::Alloc => {
                let size = params.get_usize()?;
                let align = params.get_usize()?;
                let layout = Layout::from_size_align(size, align)
                    .map_err(|_| WasmRuntimeErrorKind::InvalidParameter)?;

                return self.alloc(memory, layout).map(|v| v.get() as i32);
            }

            Function::Dealloc => {
                let base = params.get_u32()?;
                let size = params.get_usize()?;
                let align = params.get_usize()?;
                let layout = Layout::from_size_align(size, align)
                    .map_err(|_| WasmRuntimeErrorKind::InvalidParameter)?;

                println!("dealloc {:08x} {:?}", base, layout);
                let memory = memory.try_borrow()?;
                memory
                    .slice_mut(WasmPtrMut::from_u32(base), size)
                    .map(|v| v.fill(0xCC))?;

                self.malloc.lock().unwrap().dealloc(base, layout);
            }

            #[allow(unreachable_patterns)]
            _ => return Err(WasmRuntimeErrorKind::NotSupported),
        }

        Ok(0)
    }

    fn encode_io_result(
        val: Result<usize, megstd::io::Error>,
    ) -> Result<i32, WasmRuntimeErrorKind> {
        match val {
            Ok(v) => Ok(v as i32),
            Err(_err) => {
                // TODO
                Ok((-1).into())
            }
        }
    }

    fn alloc_file(&self, file: FsRawFileControlBlock) -> Result<usize, megstd::io::Error> {
        let mut vec = self.files.lock().unwrap();
        for (handle, entry) in vec.iter_mut().enumerate() {
            if entry.is_none() {
                *entry = Some(Arc::new(Mutex::new(file)));
                return Ok(handle);
            }
        }
        let handle = vec.len();
        if handle >= Self::MAX_FILES {
            return Err(megstd::io::ErrorKind::OutOfMemory.into());
        }
        vec.push(Some(Arc::new(Mutex::new(file))));
        Ok(handle)
    }

    fn close_file(&self, handle: usize) {
        let mut vec = self.files.lock().unwrap();
        if let Some(entry) = vec.get_mut(handle) {
            *entry = None;
        }
    }

    fn alloc(
        &self,
        memory: &WasmMemory,
        layout: Layout,
    ) -> Result<NonZeroU32, WasmRuntimeErrorKind> {
        let mut malloc = self.malloc.lock().unwrap();

        if let Some(result) = malloc.alloc(layout) {
            println!("alloc1 {:?} => {:08x}", layout, result);
            return Ok(result);
        } else {
            let min_alloc = WebAssembly::PAGE_SIZE;
            let delta =
                ((layout.size() + min_alloc - 1) / min_alloc) * min_alloc / WebAssembly::PAGE_SIZE;
            let new_page = memory.grow(delta as u32)?;
            if new_page > 0 {
                println!("grow {} => {}", delta, new_page);
                malloc.append_block(
                    new_page as u32 * WebAssembly::PAGE_SIZE as u32,
                    delta as u32 * WebAssembly::PAGE_SIZE as u32,
                );
            } else {
                return Err(WasmRuntimeErrorKind::OutOfMemory);
            }

            let result = match malloc.alloc(layout) {
                Some(v) => v.get(),
                None => 0,
            };
            println!("alloc2 {:?} => {:08x}", layout, result);
            NonZeroU32::new(result).ok_or(WasmRuntimeErrorKind::OutOfMemory)
        }
    }

    fn wait_key(&self, window: WindowHandle) -> Result<Option<char>, WasmRuntimeErrorKind> {
        while let Some(message) = window.clone().wait_message() {
            self.process_message(window.clone(), message);
            if self.has_to_exit.load(Ordering::Relaxed) {
                return Err(WasmRuntimeErrorKind::Exit);
            }

            if let Some(c) = self
                .read_key_buffer()
                .and_then(|v| v.key_data().map(|v| v.into_char()))
            {
                return Ok(Some(c));
            }
        }
        Err(WasmRuntimeErrorKind::TypeMismatch)
    }

    fn read_key(&self, window: WindowHandle) -> Option<char> {
        while let Some(message) = window.clone().read_message() {
            self.process_message(window.clone(), message);
        }
        self.read_key_buffer().map(|v| v.into_char())
    }

    fn read_key_buffer(&self) -> Option<KeyEvent> {
        let mut buffer = self.key_buffer.lock().unwrap();
        if buffer.len() > 0 {
            Some(buffer.remove(0))
        } else {
            None
        }
    }

    fn wait_throttle(&self, window: WindowHandle) -> Result<(), WasmRuntimeErrorKind> {
        if let Some(throttle) = self.fps_throttle.lock().unwrap().as_mut() {
            if self.throttle_timer_expired.swap(false, Ordering::Acquire) {
                return Ok(());
            }

            let next = throttle.next();
            if next.is_zero() {
                return Ok(());
            }
            window.create_timer(0, next);

            while let Some(message) = window.clone().wait_message() {
                self.process_message(window.clone(), message);
                if self.has_to_exit.load(Ordering::Relaxed) {
                    return Err(WasmRuntimeErrorKind::Exit);
                }

                if self.throttle_timer_expired.swap(false, Ordering::Acquire) {
                    return Ok(());
                }
            }
        }
        Ok(())
    }

    fn process_message(&self, window: WindowHandle, message: WindowMessage) {
        match message {
            WindowMessage::Close => {
                if self.windows.lock().unwrap().values().count() > 1 {
                    // todo:
                    window.close();
                } else {
                    self.has_to_exit.store(true, Ordering::SeqCst);
                }
            }
            WindowMessage::Key(event) => {
                event
                    .key_data()
                    .map(|data| self.key_buffer.lock().unwrap().push(data));
            }
            WindowMessage::Timer(timer) => {
                let _ = timer;
                self.throttle_timer_expired.store(true, Ordering::Release);
            }
            _ => window.handle_default_message(message),
        }
    }
}

pub struct ThrottleState {
    fps: usize,
    tick: Duration,
    tick2: Duration,
    last: Duration,
    next_min: Duration,
    fps_count: usize,
    fps_actual: usize,
}

impl ThrottleState {
    #[inline]
    pub fn new(fps: usize) -> Self {
        let now = Timer::monotonic();
        let tick = Duration::from_micros(1000_000u64.checked_div(fps as u64).unwrap_or(0));
        let tick2 = Duration::from_micros(1000_000u64.checked_div(fps as u64 / 2).unwrap_or(0));
        let next_min = now + Duration::from_secs(1);
        Self {
            fps,
            tick,
            tick2,
            last: now,
            next_min,
            fps_count: 0,
            fps_actual: 0,
        }
    }

    #[inline]
    #[allow(dead_code)]
    pub fn fps_actual(&self) -> usize {
        self.fps_actual
    }

    pub fn next(&mut self) -> Duration {
        let now = Timer::monotonic();
        let diff = if self.next_min > now {
            self.fps_count += 1;
            if self.fps_count >= self.fps {
                self.next_min - now
            } else {
                let expected = self.last + self.tick2;
                let diff = if expected > now {
                    expected - now
                } else {
                    Duration::ZERO
                };
                diff
            }
        } else {
            let expected = self.next_min + self.tick;
            self.fps_actual = self.fps_count;
            self.fps_count = 0;
            self.next_min += Duration::from_secs(1);
            let diff = if expected > now {
                expected - now
            } else {
                Duration::ZERO
            };
            diff
        };
        self.last = now;

        diff
    }
}

struct ParamsDecoder<'a> {
    args: WasmArgs<'a>,
}

impl<'a> ParamsDecoder<'a> {
    #[inline]
    pub fn new(args: WasmArgs<'a>) -> Self {
        Self { args }
    }
}

impl ParamsDecoder<'_> {
    #[inline]
    fn get_u32(&mut self) -> Result<u32, WasmRuntimeErrorKind> {
        self.args
            .next()
            .map_err(|_| WasmRuntimeErrorKind::InvalidParameter)
    }

    #[inline]
    fn get_i32(&mut self) -> Result<i32, WasmRuntimeErrorKind> {
        self.args
            .next()
            .map_err(|_| WasmRuntimeErrorKind::InvalidParameter)
    }

    #[inline]
    fn get_usize(&mut self) -> Result<usize, WasmRuntimeErrorKind> {
        self.get_u32().map(|v| v as usize)
    }

    #[inline]
    fn get_memarg(&mut self) -> Result<MemArg, WasmRuntimeErrorKind> {
        let base = self.get_u32()?;
        let len = self.get_u32()? as usize;
        Ok(MemArg::new(base, len))
    }

    #[inline]
    fn get_buffer<'a>(
        &mut self,
        memory: &'a WasmMemory,
    ) -> Result<&'a mut [u8], WasmRuntimeErrorKind> {
        let memarg = self.get_memarg()?;
        memory
            .try_borrow()
            .and_then(|v| v.slice_mut(memarg.base_mut(), memarg.len()))
    }

    #[inline]
    fn get_string<'a>(&mut self, memory: &'a WasmMemory) -> Option<&'a str> {
        let memarg = self.get_memarg().ok()?;
        memory
            .try_borrow()
            .ok()
            .and_then(|v| v.slice(memarg.base(), memarg.len()).ok())
            .and_then(|v| core::str::from_utf8(v).ok())
    }

    #[allow(dead_code)]
    #[inline]
    fn get_string16(&mut self, memory: &WasmMemory) -> Option<String> {
        let memarg = self.get_memarg().ok()?;
        memory
            .try_borrow()
            .ok()
            .and_then(|v| v.slice::<u16>(memarg.base(), memarg.len()).ok())
            .and_then(|v| String::from_utf16(v).ok())
    }

    #[inline]
    fn get_point(&mut self) -> Result<Point, WasmRuntimeErrorKind> {
        let x = self.get_i32()?;
        let y = self.get_i32()?;
        Ok(Point::new(x, y))
    }

    #[inline]
    fn get_size(&mut self) -> Result<Size, WasmRuntimeErrorKind> {
        let width = self.get_u32()?;
        let height = self.get_u32()?;
        Ok(Size::new(width, height))
    }

    #[inline]
    fn get_color(&mut self) -> Result<Color, WasmRuntimeErrorKind> {
        self.get_u32().map(|v| PackedColor::from_raw(v).into())
    }

    fn get_bitmap8<'a>(
        &mut self,
        memory: &'a WasmMemory,
    ) -> Result<BitmapRef8<'a>, WasmRuntimeErrorKind> {
        const SIZE_OF_BITMAP: usize = 5;
        let base = self.get_u32()?;
        let memory = memory.try_borrow()?;
        let array = memory.slice(WasmPtr::from_u32(base), SIZE_OF_BITMAP)?;

        let base = array[0];
        let width = array[2];
        let height = array[3];
        let _stride = array[4] as usize;

        let len = width as usize * height as usize;
        let slice = memory.slice(WasmPtr::from_u32(base), len)?;

        Ok(BitmapRef8::from_bytes(slice, Size::new(width, height)))
    }

    fn get_bitmap32<'a>(
        &mut self,
        memory: &'a WasmMemory,
    ) -> Result<BitmapRef32<'a>, WasmRuntimeErrorKind> {
        const SIZE_OF_BITMAP: usize = 5;
        let base = self.get_u32()?;
        let memory = memory.try_borrow()?;
        let array = memory.slice(WasmPtr::from_u32(base), SIZE_OF_BITMAP)?;

        let base = array[0];
        let width = array[2];
        let height = array[3];
        let _stride = array[4] as usize;

        let len = width as usize * height as usize;
        let slice = memory.slice(WasmPtr::from_u32(base), len)?;

        Ok(BitmapRef32::from_bytes(slice, Size::new(width, height)))
    }

    fn get_bitmap1<'a>(
        &mut self,
        memory: &'a WasmMemory,
    ) -> Result<OsBitmap1<'a>, WasmRuntimeErrorKind> {
        let base = self.get_u32()?;
        OsBitmap1::from_memory(memory, base)
    }

    fn get_window<'a>(
        &mut self,
        rt: &'a MyosRuntime,
    ) -> Result<&'a mut OsWindow, WasmRuntimeErrorKind> {
        let handle = self.get_usize()?;
        rt.windows
            .lock()
            .unwrap()
            .get(&handle)
            .map(|v| unsafe { &mut *v.get() })
            .ok_or(WasmRuntimeErrorKind::InvalidParameter)
    }

    fn get_file(
        &mut self,
        rt: &MyosRuntime,
    ) -> Result<Arc<Mutex<FsRawFileControlBlock>>, WasmRuntimeErrorKind> {
        let handle = self.get_usize()?;
        rt.files
            .lock()
            .unwrap()
            .get(handle)
            .and_then(|v| v.as_ref())
            .map(|v| v.clone())
            .ok_or(WasmRuntimeErrorKind::InvalidParameter)
    }
}

#[derive(Debug)]
struct MemArg {
    base: u32,
    len: usize,
}

impl MemArg {
    #[inline]
    const fn new(base: u32, len: usize) -> Self {
        Self { base, len }
    }

    #[inline]
    fn base<T>(&self) -> WasmPtr<T> {
        WasmPtr::from_u32(self.base)
    }

    #[inline]
    fn base_mut<T>(&self) -> WasmPtrMut<T> {
        WasmPtrMut::from_u32(self.base)
    }

    #[inline]
    fn len(&self) -> usize {
        self.len
    }
}

struct OsBitmap1<'a> {
    slice: &'a [u8],
    dim: Size,
    stride: usize,
}

impl<'a> OsBitmap1<'a> {
    fn from_memory(memory: &'a WasmMemory, base: u32) -> Result<Self, WasmRuntimeErrorKind> {
        const SIZE_OF_BITMAP: usize = 5;
        let memory = memory.try_borrow()?;
        let array = memory.slice(WasmPtr::from_u32(base), SIZE_OF_BITMAP)?;

        let base = array[0];
        let width = array[2];
        let height = array[3];
        let stride = array[4] as usize;

        let dim = Size::new(width, height);
        let size = stride as usize * height as usize;
        let slice = memory.slice(WasmPtr::from_u32(base), size)?;

        Ok(Self { slice, dim, stride })
    }
}

impl OsBitmap1<'_> {
    #[inline]
    const fn rect(&self, origin: Point, mode: usize) -> Rect {
        let scale = mode as u32;
        Rect {
            origin,
            size: Size::new(self.dim.width * scale, self.dim.height * scale),
        }
    }

    fn blt(&self, to: &mut BitmapRefMut, origin: Point, color: Color, mode: usize) {
        // TODO: clipping
        let scale = mode as i32;
        let stride = self.stride;
        let mut cursor = 0;
        let w8 = self.dim.width as usize / 8;
        let w7 = self.dim.width as usize & 7;
        for y in 0..self.dim.height as i32 {
            for i in 0..w8 {
                let data = unsafe { self.slice.get_unchecked(cursor + i) };
                for j in 0..8 {
                    let position = 0x80u8 >> j;
                    if (data & position) != 0 {
                        let x = scale * (i * 8 + j) as i32;
                        let y = y * scale;
                        for offset in &[(0, 0), (0, 1), (1, 0), (1, 1)] {
                            let point =
                                Point::new(origin.x + x + offset.0, origin.y + y + offset.1);
                            unsafe { to.set_pixel_unchecked(point, color) };
                        }
                    }
                }
            }
            if w7 > 0 {
                let data = unsafe { self.slice.get_unchecked(cursor + w8) };
                let base_x = w8 * 8;
                for i in 0..w7 {
                    let position = 0x80u8 >> i;
                    if (data & position) != 0 {
                        let x = scale * (i + base_x) as i32;
                        let y = y * scale;
                        for offset in &[(0, 0), (0, 1), (1, 0), (1, 1)] {
                            let point =
                                Point::new(origin.x + x + offset.0, origin.y + y + offset.1);
                            unsafe { to.set_pixel_unchecked(point, color) };
                        }
                    }
                }
            }
            cursor += stride;
        }
    }
}

struct OsWindow {
    native: WindowHandle,
    handle: usize,
    draw_region: Coordinates,
}

impl OsWindow {
    #[inline]
    const fn new(handle: usize, native: WindowHandle) -> Self {
        Self {
            native,
            handle,
            draw_region: Coordinates::void(),
        }
    }

    #[inline]
    fn native(&self) -> WindowHandle {
        self.native.clone()
    }

    #[inline]
    const fn handle(&self) -> usize {
        self.handle
    }

    #[inline]
    fn content_rect(&self) -> Rect {
        self.native.content_rect()
    }

    #[inline]
    fn begin_draw(&mut self) {
        self.draw_region = Coordinates::void();
    }

    #[inline]
    fn end_draw(&self) {
        let coords = self.draw_region;
        if coords.left <= coords.right && coords.top <= coords.bottom {
            self.native.invalidate_rect(coords.into());
        }
    }

    #[inline]
    fn add_region(&mut self, rect: Rect) {
        if let Ok(coords) = Coordinates::from_rect(rect) {
            self.draw_region.merge(coords);
        }
    }

    #[inline]
    fn draw_in_rect<F>(&mut self, rect: Rect, f: F)
    where
        F: FnOnce(&mut BitmapRefMut) -> (),
    {
        let _ = self.native.draw_in_rect(rect, f);
        self.add_region(rect);
    }
}

impl Drop for OsWindow {
    #[inline]
    fn drop(&mut self) {
        self.native.close();
    }
}

#[allow(dead_code)]
pub struct SimpleAllocator {
    data: Vec<SimpleFreePair>,
    strategy: AllocationStrategy,
}

#[allow(dead_code)]
impl SimpleAllocator {
    const MIN_MASK: u32 = 0x0000_000F;

    #[inline]
    pub const fn new(strategy: AllocationStrategy) -> Self {
        Self {
            data: Vec::new(),
            strategy,
        }
    }

    fn merge(&mut self, new_data: Option<SimpleFreePair>) {
        if let Some(new_data) = new_data {
            self.data.push(new_data);
        }
        self.data.sort_by_key(|v| v.base);

        let mut do_retry = false;
        let mut retry_index = 0;
        loop {
            for index in retry_index..self.data.len() - 1 {
                let current = unsafe { self.data.get_unchecked(index) };
                let next = unsafe { self.data.get_unchecked(index + 1) };
                if current.next_base() == next.base {
                    let next = self.data.remove(index + 1);
                    self.data[index].size += next.size;
                    retry_index = index;
                    do_retry = true;
                    break;
                }
            }
            if do_retry == false {
                break;
            }
        }
    }

    pub fn append_block(&mut self, base: u32, size: u32) {
        self.merge(Some(SimpleFreePair::new(base, size)));
    }

    pub fn alloc(&mut self, layout: Layout) -> Option<NonZeroU32> {
        let layout_align = u32::max(layout.align() as u32, Self::MIN_MASK + 1);
        let layout_mask = layout_align - 1;
        let min_alloc =
            (u32::max(layout_align, layout.size() as u32) + Self::MIN_MASK) & !Self::MIN_MASK;
        let max_alloc = (min_alloc + layout_mask) & layout_mask;

        let mut result = 0;
        let mut extend = Vec::new();
        match self.strategy {
            AllocationStrategy::FirstFit => {
                for pair in &mut self.data {
                    if (pair.base & layout_mask) == 0 && pair.size >= min_alloc {
                        result = pair.base;
                        pair.size -= min_alloc;
                        pair.base += min_alloc;
                        break;
                    } else if pair.size >= max_alloc {
                        let redundant = pair.base & layout_mask;
                        extend.push(SimpleFreePair::new(pair.base, redundant));
                        pair.base -= redundant;
                        pair.size -= redundant;

                        result = pair.base;
                        pair.size -= min_alloc;
                        pair.base += min_alloc;
                        break;
                    }
                }
            }
            AllocationStrategy::BestFit => todo!(),
        }
        if extend.len() > 0 {
            self.data.extend_from_slice(extend.as_slice());
            self.merge(None);
        }

        NonZeroU32::new(result)
    }

    pub fn dealloc(&mut self, base: u32, layout: Layout) {
        let layout_align = u32::max(layout.align() as u32, Self::MIN_MASK + 1);
        // let layout_mask = layout_align - 1;
        let min_alloc =
            (u32::max(layout_align, layout.size() as u32) + Self::MIN_MASK) & !Self::MIN_MASK;

        let new_pair = SimpleFreePair::new(base, min_alloc);

        let mut cursor = None;
        for (index, pair) in self.data.iter_mut().enumerate() {
            if new_pair.next_base() == pair.base {
                pair.base = new_pair.base;
                pair.size += min_alloc;
                cursor = Some(index);
                break;
            } else if pair.next_base() == base {
                pair.size += min_alloc;
                cursor = Some(index);
                break;
            }
        }

        if let Some(index) = cursor {
            if index < self.data.len() - 1 {
                let current = unsafe { self.data.get_unchecked(index) };
                let next = unsafe { self.data.get_unchecked(index + 1) };
                if current.next_base() == next.base {
                    let next = self.data.remove(index + 1);
                    self.data[index].size += next.size;
                }
            }
        } else {
            self.merge(Some(new_pair));
        }
        for data in &self.data {
            println!("DATA {:08x} {}", data.base, data.size);
        }
    }
}

impl Default for SimpleAllocator {
    #[inline]
    fn default() -> Self {
        Self::new(AllocationStrategy::FirstFit)
    }
}

#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[allow(dead_code)]
pub enum AllocationStrategy {
    FirstFit,
    BestFit,
}

#[derive(Clone, Copy)]
struct SimpleFreePair {
    base: u32,
    size: u32,
}

impl SimpleFreePair {
    #[inline]
    pub const fn new(base: u32, size: u32) -> Self {
        Self { base, size }
    }

    #[inline]
    pub const fn next_base(&self) -> u32 {
        self.base + self.size
    }
}
