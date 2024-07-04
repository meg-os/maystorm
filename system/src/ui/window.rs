use super::font::*;
use super::text::*;
use super::theme::Theme;
use crate::init::SysInit;
use crate::io::{hid_mgr::*, screen::Screen};
use crate::res::icon::IconManager;
use crate::sync::{
    atomic::AtomicFlags,
    RwLock,
    {fifo::*, semaphore::*, spinlock::SpinMutex},
};
use crate::system::System;
use crate::task::scheduler::*;
use crate::*;
use core::cell::UnsafeCell;
use core::future::Future;
use core::num::*;
use core::ops::Deref;
use core::pin::Pin;
use core::sync::atomic::*;
use core::task::{Context, Poll};
use core::time::Duration;
use futures_util::task::AtomicWaker;
use megstd::{drawing::*, io::hid::*, sys::megos};
use paste::paste;

const MAX_WINDOWS: usize = 255;
const WINDOW_SYSTEM_EVENT_QUEUE_SIZE: usize = 100;

const WINDOW_BORDER_WIDTH: u32 = 1;
const WINDOW_CORNER_RADIUS: u32 = 8;
const WINDOW_THICK_BORDER_WIDTH_V: u32 = WINDOW_CORNER_RADIUS / 2;
const WINDOW_THICK_BORDER_WIDTH_H: u32 = WINDOW_CORNER_RADIUS / 2;
const WINDOW_TITLE_HEIGHT: u32 = 28;
const WINDOW_TITLE_BORDER: u32 = 1;
const WINDOW_SHADOW_PADDING: u32 = 16;
const SHADOW_RADIUS: u32 = 12;
const SHADOW_OFFSET: Point = Point::new(2, 2);
const SHADOW_LEVEL: usize = 96;

const CORNER_MASK: [u8; WINDOW_CORNER_RADIUS as usize] = [6, 4, 3, 2, 1, 1, 0, 0];

static mut WM: Option<Box<WindowManager<'static>>> = None;

pub struct WindowManager<'a> {
    sem_event: Semaphore,
    attributes: AtomicFlags<WindowManagerAttributes>,
    system_event: ConcurrentFifo<WindowSystemEvent>,

    pointer_hotspot: Point,
    pointer_x: AtomicIsize,
    pointer_y: AtomicIsize,
    buttons: AtomicFlags<MouseButton>,
    buttons_down: AtomicFlags<MouseButton>,
    buttons_up: AtomicFlags<MouseButton>,

    screen_size: Size,
    screen_insets: SpinMutex<EdgeInsets>,
    update_coords: SpinMutex<Coordinates>,

    resources: Resources<'a>,

    window_pool: RwLock<BTreeMap<WindowHandle, Arc<UnsafeCell<RawWindow>>>>,
    window_orders: RwLock<Vec<WindowHandle>>,

    root: WindowHandle,
    pointer: WindowHandle,
    barrier: WindowHandle,

    active: RwLock<Option<WindowHandle>>,
    captured: RwLock<Option<WindowHandle>>,
    entered: RwLock<Option<WindowHandle>>,
}

#[allow(dead_code)]
struct Resources<'a> {
    window_button_width: u32,
    close_button: OperationalBitmap,
    back_button: OperationalBitmap,
    title_font: FontDescriptor,
    label_font: FontDescriptor,
    _phantom: &'a (),
}

macro_rules! rwlock_helper {
    { $( $vis:vis fn $name:ident ( $self:ident ) -> $type:ty; )* } => {
        $(
            paste! {
                $vis fn $name (& $self) -> $type {
                    $self.$name.read().unwrap().clone()
                }

                $vis fn [<set_ $name>] (& $self, value: $type) {
                    let mut p = $self.$name.write().unwrap();
                    *p = value;
                }
            }
        )*
    };
}

impl WindowManager<'static> {
    pub fn init(main_screen: Arc<dyn Screen<BitmapRef32<'static>, ColorType = TrueColor>>) {
        assert_call_once!();

        let screen_size = main_screen.size();
        let pointer_x = (screen_size.width / 2) as i32;
        let pointer_y = (screen_size.height / 2) as i32;
        let mut window_pool = BTreeMap::new();
        let mut window_orders = Vec::with_capacity(MAX_WINDOWS);

        let window_button_width = WINDOW_TITLE_HEIGHT;
        let close_button = IconManager::mask(r::Icons::Close).unwrap();
        let back_button = IconManager::mask(r::Icons::ChevronLeft).unwrap();

        let root = {
            let window = RawWindowBuilder::new()
                .style(WindowStyle::OPAQUE | WindowStyle::NO_SHADOW)
                .level(WindowLevel::ROOT)
                .frame(Rect::from(screen_size))
                .bg_color(Color::from_rgb(0x000000))
                .without_message_queue()
                .build_inner("Desktop");

            let handle = window.handle.clone();
            window_pool.insert(handle.clone(), Arc::new(UnsafeCell::new(window)));
            handle
        };
        window_orders.push(root.clone());

        let (pointer, pointer_hotspot) = {
            let pointer_image =
                OwnedBitmap::Argb32(IconManager::bitmap(r::Icons::Pointer).unwrap());
            let pointer_size = pointer_image.size();
            let window = RawWindowBuilder::new()
                .style(WindowStyle::NO_SHADOW)
                .level(WindowLevel::POINTER)
                .size(pointer_size)
                .bg_color(Color::TRANSPARENT)
                .without_message_queue()
                .build_inner("Pointer");

            window
                .draw_in_rect(pointer_size.into(), |bitmap| {
                    bitmap.blt(
                        &pointer_image.as_const(),
                        Point::new(0, 0),
                        pointer_size.into(),
                    )
                })
                .unwrap();

            let handle = window.handle.clone();
            window_pool.insert(handle.clone(), Arc::new(UnsafeCell::new(window)));
            (handle, Point::new(10, 6))
        };

        let barrier = {
            let window = RawWindowBuilder::new()
                .style(WindowStyle::NO_SHADOW)
                .level(WindowLevel::POPUP_BARRIER)
                .frame(Rect::from(screen_size))
                .bg_color(Color::TRANSPARENT)
                .without_message_queue()
                .bitmap_strategy(BitmapStrategy::NonBitmap)
                .build_inner("Barrier");

            let handle = window.handle.clone();
            window_pool.insert(handle.clone(), Arc::new(UnsafeCell::new(window)));
            handle
        };

        unsafe {
            WM = Some(Box::new(WindowManager {
                sem_event: Semaphore::new(0),
                attributes: AtomicFlags::default(),
                pointer_hotspot,
                pointer_x: AtomicIsize::new(pointer_x as isize),
                pointer_y: AtomicIsize::new(pointer_y as isize),
                buttons: AtomicFlags::empty(),
                buttons_down: AtomicFlags::empty(),
                buttons_up: AtomicFlags::empty(),
                screen_size,
                screen_insets: SpinMutex::new(EdgeInsets::default()),
                update_coords: SpinMutex::new(Coordinates::VOID),
                resources: Resources {
                    _phantom: &(),
                    close_button,
                    window_button_width,
                    back_button,
                    title_font: FontManager::title_font(),
                    label_font: FontManager::ui_font(),
                },
                window_pool: RwLock::new(window_pool),
                window_orders: RwLock::new(window_orders),
                root,
                pointer,
                barrier,
                active: RwLock::new(None),
                captured: RwLock::new(None),
                entered: RwLock::new(None),
                system_event: ConcurrentFifo::with_capacity(WINDOW_SYSTEM_EVENT_QUEUE_SIZE),
            }));
        }

        SpawnOption::with_priority(Priority::High)
            .start(Self::window_thread, 0, "Window Manager")
            .unwrap();
    }

    #[track_caller]
    fn add(window: RawWindow) {
        let handle = window.handle.clone();
        WindowManager::shared()
            .window_pool
            .write()
            .unwrap()
            .insert(handle, Arc::new(UnsafeCell::new(window)));
    }

    fn remove(window: &RawWindow) {
        window.hide();
        let shared = WindowManager::shared();
        let window_orders = shared.window_orders.write().unwrap();
        let handle = window.handle.clone();
        shared.window_pool.write().unwrap().remove(&handle);
        drop(window_orders);
    }

    #[inline]
    fn _get(&self, key: &WindowHandle) -> Option<WindowRef> {
        WindowManager::shared()
            .window_pool
            .read()
            .unwrap()
            .get(key)
            .map(|v| WindowRef(v.clone()))
    }

    #[inline]
    fn update<F, R>(&self, key: &WindowHandle, f: F) -> Option<R>
    where
        F: FnOnce(&mut RawWindow) -> R,
    {
        self.window_pool.read().unwrap().get(key).map(|v| unsafe {
            let window = v.clone().get();
            f(&mut *window)
        })
    }
}

impl WindowManager<'_> {
    #[inline]
    #[track_caller]
    fn shared<'a>() -> &'a WindowManager<'static> {
        unsafe { WM.as_ref().unwrap() }
    }

    #[inline]
    fn shared_opt<'a>() -> Option<&'a Box<WindowManager<'static>>> {
        unsafe { WM.as_ref() }
    }

    /// Window Manager's Thread
    fn window_thread(_: usize) {
        let shared = WindowManager::shared();

        let mut captured_offset = Point::default();

        loop {
            shared.sem_event.wait();

            if shared
                .attributes
                .fetch_reset(WindowManagerAttributes::EVENT)
            {
                while let Some(event) = shared.system_event.dequeue() {
                    match event {
                        WindowSystemEvent::Key(w, e) => {
                            w.post(WindowMessage::Key(e)).unwrap();
                        }
                    }
                }
            }
            if shared
                .attributes
                .fetch_reset(WindowManagerAttributes::EVENT_MOUSE_SHOW)
            {
                if shared.attributes.contains(
                    WindowManagerAttributes::POINTER_ENABLED
                        | WindowManagerAttributes::POINTER_VISIBLE,
                ) && !shared
                    .attributes
                    .contains(WindowManagerAttributes::POINTER_HIDE_TEMP)
                {
                    shared.pointer.show();
                } else {
                    shared.pointer.hide();
                }
            }
            if shared
                .attributes
                .fetch_reset(WindowManagerAttributes::EVENT_MOUSE_MOVE)
            {
                if Self::is_pointer_enabled() {
                    let position = shared.pointer();
                    let current_buttons = shared.buttons.value();
                    let buttons_down = shared.buttons_down.swap(MouseButton::empty());
                    let buttons_up = shared.buttons_up.swap(MouseButton::empty());

                    if let Some(captured) = shared.captured() {
                        if current_buttons.contains(MouseButton::PRIMARY) {
                            if shared
                                .attributes
                                .contains(WindowManagerAttributes::CLOSE_DOWN)
                            {
                                let _ = captured.update_opt(|window| {
                                    if window.test_frame(position, window.close_button_frame()) {
                                        window.set_close_state(ViewActionState::Pressed);
                                    } else {
                                        window.set_close_state(ViewActionState::Normal);
                                    }
                                });
                            } else if shared
                                .attributes
                                .contains(WindowManagerAttributes::BACK_DOWN)
                            {
                                let _ = captured.update_opt(|window| {
                                    if window.test_frame(position, window.back_button_frame()) {
                                        window.set_back_state(ViewActionState::Pressed);
                                    } else {
                                        window.set_back_state(ViewActionState::Normal);
                                    }
                                });
                            } else if shared.attributes.contains(WindowManagerAttributes::MOVING) {
                                let screen_insets = shared.screen_insets.lock();
                                // dragging title
                                let top = if captured.as_ref().level < WindowLevel::FLOATING {
                                    screen_insets.top
                                } else {
                                    0
                                };
                                let bottom = (shared.screen_size.height() - WINDOW_TITLE_HEIGHT / 2)
                                    as i32
                                    - if captured.as_ref().level < WindowLevel::FLOATING {
                                        screen_insets.bottom
                                    } else {
                                        0
                                    };
                                let x = position.x - captured_offset.x;
                                let y = (position.y - captured_offset.y).max(top).min(bottom);
                                captured.move_to(Point::new(x, y));
                            } else {
                                let _ = Self::make_mouse_events(
                                    captured,
                                    position,
                                    current_buttons,
                                    buttons_down,
                                    buttons_up,
                                );
                            }
                        } else {
                            if shared
                                .attributes
                                .contains(WindowManagerAttributes::CLOSE_DOWN)
                            {
                                let _ = captured.update_opt(|window| {
                                    window.set_close_state(ViewActionState::Normal);
                                    if window.test_frame(position, window.close_button_frame()) {
                                        let _ = captured.post(WindowMessage::Close);
                                    }
                                });
                            } else if shared
                                .attributes
                                .contains(WindowManagerAttributes::BACK_DOWN)
                            {
                                let _ = captured.update_opt(|window| {
                                    window.set_back_state(ViewActionState::Normal);
                                    if window.test_frame(position, window.back_button_frame()) {
                                        let _ = captured.post(WindowMessage::Back);
                                    }
                                });
                            } else {
                                let _ = Self::make_mouse_events(
                                    captured.clone(),
                                    position,
                                    current_buttons,
                                    buttons_down,
                                    buttons_up,
                                );
                            }

                            shared.set_captured(None);
                            shared.attributes.remove(
                                WindowManagerAttributes::MOVING
                                    | WindowManagerAttributes::CLOSE_DOWN
                                    | WindowManagerAttributes::BACK_DOWN,
                            );

                            let target = Self::window_at_point(position);
                            if let Some(entered) = shared.entered() {
                                if entered != target {
                                    let _ = Self::make_mouse_events(
                                        captured,
                                        position,
                                        current_buttons,
                                        MouseButton::empty(),
                                        MouseButton::empty(),
                                    );
                                    shared
                                        .make_enver_and_leave_event(
                                            target,
                                            entered,
                                            position,
                                            current_buttons,
                                        )
                                        .unwrap();
                                }
                            }
                        }
                    } else {
                        let target = Self::window_at_point(position).clone();

                        if buttons_down.contains(MouseButton::PRIMARY) {
                            if let Some(active) = shared.active() {
                                if active != target {
                                    WindowManager::make_active(Some(target.clone()));
                                }
                            } else {
                                WindowManager::make_active(Some(target.clone()));
                            }

                            let target_window = target.clone().as_ref();
                            if target_window.close_button_state != ViewActionState::Disabled
                                && target_window
                                    .test_frame(position, target_window.close_button_frame())
                            {
                                let _ = target.update_opt(|window| {
                                    window.set_close_state(ViewActionState::Pressed)
                                });
                                shared
                                    .attributes
                                    .insert(WindowManagerAttributes::CLOSE_DOWN);
                            } else if target_window.back_button_state != ViewActionState::Disabled
                                && target_window
                                    .test_frame(position, target_window.back_button_frame())
                            {
                                let _ = target.update_opt(|window| {
                                    window.set_back_state(ViewActionState::Pressed)
                                });
                                shared.attributes.insert(WindowManagerAttributes::BACK_DOWN);
                            } else if target_window.style.contains(WindowStyle::PINCHABLE) {
                                shared.attributes.insert(WindowManagerAttributes::MOVING);
                            } else {
                                if target_window.test_frame(position, target_window.title_frame()) {
                                    shared.attributes.insert(WindowManagerAttributes::MOVING);
                                } else {
                                    let _ = Self::make_mouse_events(
                                        target.clone(),
                                        position,
                                        current_buttons,
                                        buttons_down,
                                        buttons_up,
                                    );
                                }
                            }
                            shared.set_captured(Some(target.clone()));
                            captured_offset = position - target_window.visible_frame().origin();
                        } else {
                            let _ = Self::make_mouse_events(
                                target.clone(),
                                position,
                                current_buttons,
                                buttons_down,
                                buttons_up,
                            );
                        }

                        if let Some(entered) = shared.entered() {
                            if entered != target {
                                shared
                                    .make_enver_and_leave_event(
                                        target,
                                        entered,
                                        position,
                                        current_buttons,
                                    )
                                    .unwrap();
                            }
                        }
                    }

                    shared.pointer.move_to(position - shared.pointer_hotspot);
                }
            }
            if shared
                .attributes
                .fetch_reset(WindowManagerAttributes::NEEDS_REDRAW)
            {
                let mut update_coords = shared.update_coords.lock();
                if update_coords.is_valid() {
                    let coords = *update_coords;
                    *update_coords = Coordinates::VOID;
                    drop(update_coords);
                    shared.root.as_ref().draw_inner_to_screen(coords.into());
                }
            }
        }
    }

    #[inline]
    fn signal(&self, flag: WindowManagerAttributes) {
        self.attributes.insert(flag);
        self.sem_event.signal();
    }

    #[inline]
    fn post_system_event(event: WindowSystemEvent) -> Result<(), WindowSystemEvent> {
        let shared = Self::shared();
        let r = shared.system_event.enqueue(event);
        shared.signal(WindowManagerAttributes::EVENT);
        r
    }

    fn make_mouse_events(
        target: WindowHandle,
        position: Point,
        buttons: MouseButton,
        down: MouseButton,
        up: MouseButton,
    ) -> Result<(), WindowPostError> {
        let window = target.as_ref();
        let origin = window.frame.insets_by(window.content_insets).origin();
        let point = Point::new(position.x - origin.x, position.y - origin.y);

        if down.is_empty() && up.is_empty() {
            return target.post(WindowMessage::MouseMove(MouseEvent::new(
                point,
                buttons,
                MouseButton::empty(),
            )));
        }
        let mut errors = None;
        if !down.is_empty() {
            match target.post(WindowMessage::MouseDown(MouseEvent::new(
                point, buttons, down,
            ))) {
                Ok(_) => (),
                Err(err) => errors = Some(err),
            };
        }
        if !up.is_empty() {
            match target.post(WindowMessage::MouseUp(MouseEvent::new(point, buttons, up))) {
                Ok(_) => (),
                Err(err) => errors = Some(err),
            };
        }
        match errors {
            Some(err) => Err(err),
            None => Ok(()),
        }
    }

    fn make_enver_and_leave_event(
        &self,
        new: WindowHandle,
        old: WindowHandle,
        position: Point,
        buttons: MouseButton,
    ) -> Result<(), WindowPostError> {
        self.set_entered(Some(new.clone()));
        old.post(WindowMessage::MouseLeave(MouseEvent::new(
            position,
            buttons,
            MouseButton::empty(),
        )))?;
        new.post(WindowMessage::MouseEnter(MouseEvent::new(
            position,
            buttons,
            MouseButton::empty(),
        )))?;

        Ok(())
    }

    #[inline]
    pub fn is_enabled() -> bool {
        unsafe { WM.is_some() }
    }

    #[inline]
    fn next_window_handle() -> WindowHandle {
        static NEXT_ID: AtomicUsize = AtomicUsize::new(1);
        WindowHandle::new(NEXT_ID.fetch_add(1, Ordering::SeqCst)).unwrap()
    }

    fn add_hierarchy(window: WindowHandle) {
        let Some(window) = window.get() else { return };

        Self::remove_hierarchy(window.handle.clone());
        let mut window_orders = WindowManager::shared().window_orders.write().unwrap();

        let mut insert_position = None;
        for (index, lhs) in window_orders.iter().enumerate() {
            if lhs.as_ref().level > window.level {
                insert_position = Some(index);
                break;
            }
        }
        if let Some(insert_position) = insert_position {
            window_orders.insert(insert_position, window.handle.clone());
        } else {
            window_orders.push(window.handle.clone());
        }

        window.attributes.insert(WindowAttributes::VISIBLE);

        drop(window_orders);
    }

    fn remove_hierarchy(window: WindowHandle) {
        let Some(window) = window.get() else { return };

        window.attributes.remove(WindowAttributes::VISIBLE);

        let mut window_orders = WindowManager::shared().window_orders.write().unwrap();
        let mut remove_position = None;
        for (index, lhs) in window_orders.iter().enumerate() {
            if *lhs == window.handle {
                remove_position = Some(index);
                break;
            }
        }
        if let Some(remove_position) = remove_position {
            window_orders.remove(remove_position);
        }

        drop(window_orders);
    }

    #[inline]
    pub fn main_screen_bounds() -> Rect {
        let shared = Self::shared();
        shared.screen_size.into()
    }

    #[inline]
    pub fn user_screen_bounds() -> Rect {
        match WindowManager::shared_opt() {
            Some(shared) => Rect::from(shared.screen_size).insets_by(*shared.screen_insets.lock()),
            None => System::main_screen().unwrap().bounds(),
        }
    }

    #[inline]
    pub fn screen_insets() -> EdgeInsets {
        *Self::shared().screen_insets.lock()
    }

    #[inline]
    pub fn add_screen_insets(insets: EdgeInsets) {
        let mut screen_insets = Self::shared().screen_insets.lock();
        *screen_insets += insets;
    }

    #[inline]
    pub fn invalidate_screen(rect: Rect) {
        let shared = Self::shared();
        let mut update_coords = shared.update_coords.lock();
        if let Ok(coords) = Coordinates::from_rect(rect) {
            update_coords.merge(coords);
            shared.signal(WindowManagerAttributes::NEEDS_REDRAW);
        }
    }

    fn make_active(window: Option<WindowHandle>) {
        let shared = WindowManager::shared();
        if let Some(old_active) = shared.active() {
            let _ = old_active.post(WindowMessage::Deactivated);
            shared.set_active(window.clone());
            let _ = old_active.update_opt(|window| window.refresh_title());
        } else {
            shared.set_active(window.clone());
        }
        if let Some(active) = window {
            let _ = active.post(WindowMessage::Activated);
            active.show();
        }
    }

    fn window_at_point(point: Point) -> WindowHandle {
        let shared = WindowManager::shared();
        let window_orders = shared.window_orders.read().unwrap();
        for handle in window_orders.iter().rev().skip(1) {
            let window = handle.as_ref();
            if window.frame.contains(point) {
                return handle.clone();
            }
        }
        shared.root.clone()
    }

    fn pointer(&self) -> Point {
        Point::new(
            self.pointer_x.load(Ordering::Relaxed) as i32,
            self.pointer_y.load(Ordering::Relaxed) as i32,
        )
    }

    fn _update_relative_coord(
        coord: &AtomicIsize,
        delta: i32,
        min_value: i32,
        max_value: i32,
    ) -> bool {
        match coord.fetch_update(Ordering::SeqCst, Ordering::Relaxed, |old_value| {
            let new_value = (old_value + delta as isize)
                .max(min_value as isize)
                .min(max_value as isize);
            if old_value == new_value {
                None
            } else {
                Some(new_value)
            }
        }) {
            Ok(_) => true,
            Err(_) => false,
        }
    }

    fn _update_absolute_coord(
        coord: &AtomicIsize,
        new_value: i32,
        min_value: i32,
        max_value: i32,
    ) -> bool {
        match coord.fetch_update(Ordering::SeqCst, Ordering::Relaxed, |old_value| {
            let new_value = (new_value as isize)
                .max(min_value as isize)
                .min(max_value as isize);
            if old_value == new_value {
                None
            } else {
                Some(new_value)
            }
        }) {
            Ok(_) => true,
            Err(_) => false,
        }
    }

    fn _process_buttons(pointer_state: &MouseState) -> bool {
        let Some(shared) = Self::shared_opt() else {
            return false;
        };

        let current_buttons = pointer_state.current_buttons.value();
        let prev_buttons = pointer_state.prev_buttons.value();

        let button_changes = current_buttons ^ prev_buttons;
        let button_down = button_changes & current_buttons;
        let button_up = button_changes & prev_buttons;
        let button_changed = !button_changes.is_empty();

        if button_changed {
            shared.buttons.store(current_buttons);
            shared.buttons_down.fetch_or(button_down);
            shared.buttons_up.fetch_or(button_up);
        }

        button_changed
    }

    pub fn post_relative_pointer(pointer_state: &MouseState) {
        let Some(shared) = Self::shared_opt() else {
            return;
        };
        let button_changed = Self::_process_buttons(pointer_state);

        let screen_bounds: Rect = shared.screen_size.into();

        let pointer = Point::new(
            pointer_state.x.swap(0, Ordering::SeqCst) as i32,
            pointer_state.y.swap(0, Ordering::SeqCst) as i32,
        );

        let moved = Self::_update_relative_coord(
            &shared.pointer_x,
            pointer.x,
            screen_bounds.min_x(),
            screen_bounds.width() as i32 - 1,
        ) | Self::_update_relative_coord(
            &shared.pointer_y,
            pointer.y,
            screen_bounds.min_y(),
            screen_bounds.height() as i32 - 1,
        );

        if button_changed | moved {
            WindowManager::set_pointer_move();
        }
    }

    pub fn post_absolute_pointer(pointer_state: &MouseState) {
        let Some(shared) = Self::shared_opt() else {
            return;
        };
        let button_changed = Self::_process_buttons(pointer_state);

        let screen_bounds: Rect = shared.screen_size.into();

        let pointer_x = screen_bounds.width() as i32
            * pointer_state.x.load(Ordering::Relaxed) as i32
            / pointer_state.max_x;
        let pointer_y = screen_bounds.height() as i32
            * pointer_state.y.load(Ordering::Relaxed) as i32
            / pointer_state.max_y;

        let moved = Self::_update_absolute_coord(
            &shared.pointer_x,
            pointer_x,
            screen_bounds.min_x(),
            screen_bounds.width() as i32 - 1,
        ) | Self::_update_absolute_coord(
            &shared.pointer_y,
            pointer_y,
            screen_bounds.min_y(),
            screen_bounds.height() as i32 - 1,
        );

        if button_changed | moved {
            WindowManager::set_pointer_move();
        }
    }

    pub fn post_key_event(event: KeyEvent) {
        let Some(shared) = Self::shared_opt() else {
            return;
        };
        if event.usage() == Usage::DELETE
            && event.modifier().has_ctrl()
            && event.modifier().has_alt()
        {
            // ctrl alt del
            SysInit::system_reset(false);
        } else if let Some(window) = shared.active() {
            Self::post_system_event(WindowSystemEvent::Key(window, event)).unwrap();
        }
    }

    #[inline]
    pub fn current_desktop_window() -> WindowHandle {
        Self::shared().root.clone()
    }

    pub fn set_desktop_color(color: Color) {
        let desktop = Self::shared().root.clone();
        desktop.update(|window| {
            window.set_bg_color(color);
        });
    }

    pub fn set_desktop_bitmap<'a>(bitmap: &BitmapRef) {
        let shared = Self::shared();
        let _ = shared.root.update_opt(|root| {
            let (mut r, mut g, mut b, mut a) = (0, 0, 0, 0);
            for pixel in bitmap.all_pixels() {
                let c = pixel.into_true_color().components();
                r += c.r as usize;
                g += c.g as usize;
                b += c.b as usize;
                a += c.a.as_usize();
            }
            let total_pixels = bitmap.width() as usize * bitmap.height() as usize;
            let tint_color = Color::Argb32(TrueColor::from(ColorComponents::from_rgba(
                r.checked_div(total_pixels).unwrap_or_default() as u8,
                g.checked_div(total_pixels).unwrap_or_default() as u8,
                b.checked_div(total_pixels).unwrap_or_default() as u8,
                Alpha8::new(a.checked_div(total_pixels).unwrap_or_default() as u8),
            )));

            root.set_bg_color(tint_color);
            let target = root.bitmap();
            if target.size() == bitmap.size() {
                target.blt_transparent(
                    bitmap,
                    Point::zero(),
                    bitmap.bounds(),
                    IndexedColor::KEY_COLOR,
                );
            } else {
                match bitmap {
                    BitmapRef::Indexed(_) => (),
                    BitmapRef::Argb32(bitmap) => {
                        let target_width = target.width() as f64;
                        let target_height = target.height() as f64;
                        let mut new_width = target_width;
                        let mut new_height =
                            new_width * bitmap.height() as f64 / bitmap.width() as f64;
                        if new_height > target_height {
                            new_height = target_height;
                            new_width = new_height * bitmap.width() as f64 / bitmap.height() as f64;
                        }
                        let new_size = Size::new(new_width as u32, new_height as u32);
                        let Ok(new_bitmap) = bitmap.scale(new_size) else {
                            return;
                        };
                        let origin = Point::new(
                            (target.bounds().width() as i32 - new_size.width() as i32) / 2,
                            (target.bounds().height() as i32 - new_size.height() as i32) / 2,
                        );
                        target.blt_transparent(
                            &BitmapRef::from(new_bitmap.as_ref()),
                            origin,
                            new_size.bounds(),
                            IndexedColor::KEY_COLOR,
                        );
                    }
                }
            }

            root.set_needs_display();
        });
    }

    #[inline]
    pub fn is_pointer_enabled() -> bool {
        Self::shared()
            .attributes
            .contains(WindowManagerAttributes::POINTER_ENABLED)
    }

    pub fn set_pointer_enabled(enabled: bool) -> bool {
        let result = Self::is_pointer_enabled();
        let shared = Self::shared();
        shared
            .attributes
            .set(WindowManagerAttributes::POINTER_ENABLED, enabled);
        if !enabled {
            shared.pointer.hide();
        }
        shared.signal(WindowManagerAttributes::EVENT_MOUSE_SHOW);
        result
    }

    #[inline]
    pub fn is_pointer_visible() -> bool {
        Self::shared()
            .attributes
            .contains(WindowManagerAttributes::POINTER_VISIBLE)
    }

    pub fn set_pointer_visible(visible: bool) -> bool {
        let result = Self::is_pointer_visible();
        let shared = Self::shared();
        shared
            .attributes
            .set(WindowManagerAttributes::POINTER_VISIBLE, visible);
        shared
            .attributes
            .remove(WindowManagerAttributes::POINTER_HIDE_TEMP);
        if !visible {
            shared.pointer.hide();
        }
        shared.signal(WindowManagerAttributes::EVENT_MOUSE_SHOW);
        result
    }

    /// Make the pointer temporarily invisible
    pub fn hide_pointer_temporarily() {
        let shared = Self::shared();
        shared
            .attributes
            .insert(WindowManagerAttributes::POINTER_HIDE_TEMP);
        shared.pointer.hide();
    }

    #[inline]
    pub fn set_pointer_move() {
        let shared = Self::shared();
        if shared
            .attributes
            .fetch_reset(WindowManagerAttributes::POINTER_HIDE_TEMP)
        {
            shared
                .attributes
                .insert(WindowManagerAttributes::EVENT_MOUSE_SHOW);
        }
        shared.signal(WindowManagerAttributes::EVENT_MOUSE_MOVE);
    }

    #[inline]
    pub fn set_pointer_states(is_enabled: bool, is_visible: bool, is_temporarily_hidden: bool) {
        let shared = Self::shared();
        let _ = shared.attributes.fetch_update(|attr| {
            let mut attr = attr;
            attr.set(WindowManagerAttributes::POINTER_ENABLED, is_enabled);
            attr.set(WindowManagerAttributes::POINTER_VISIBLE, is_visible);
            attr.set(
                WindowManagerAttributes::POINTER_HIDE_TEMP,
                is_temporarily_hidden,
            );
            Some(attr)
        });
        if !is_enabled || !is_visible || is_temporarily_hidden {
            shared.pointer.hide();
        }
        shared
            .attributes
            .insert(WindowManagerAttributes::EVENT_MOUSE_SHOW);
    }

    #[inline]
    pub fn while_hiding_pointer<F, R>(f: F) -> R
    where
        F: FnOnce() -> R,
    {
        let state = Self::set_pointer_visible(false);
        let result = f();
        Self::set_pointer_visible(state);
        result
    }

    pub fn save_screen_to(bitmap: &mut BitmapRefMut32, rect: Rect) {
        let shared = Self::shared();
        Self::while_hiding_pointer(|| shared.root.draw_into(bitmap, rect));
    }

    pub fn get_statistics(sb: &mut String) {
        let shared = Self::shared();

        writeln!(sb, "  # PID Lv Frame",).unwrap();
        for window in shared.window_pool.read().unwrap().values() {
            let window = unsafe { &*window.clone().as_ref().get() };
            let frame = window.frame;
            writeln!(
                sb,
                "{:3} {:3} {:2x} {:4} {:4} {:4} {:4} {}",
                window.handle.0,
                usize::from(window.pid),
                window.level.0,
                frame.min_x(),
                frame.min_y(),
                frame.width(),
                frame.height(),
                window.title(),
            )
            .unwrap();
        }
    }

    pub fn set_barrier_opacity(opacity: Alpha8) {
        let shared = Self::shared();
        let barrier = shared.barrier.clone();
        if opacity.is_transparent() {
            barrier.hide();
        } else {
            let color = TrueColor::from_gray(0, opacity);
            barrier.set_bg_color(color.into());
            if !barrier.is_visible() {
                barrier.show();
            }
        }
    }

    fn _create_timer(
        handle: &WindowHandle,
        timer_type: WindowTimerType,
        timer_id: usize,
        duration: Duration,
    ) {
        let Some(window) = handle.validate() else {
            return;
        };
        let payload = WindowTimerEvent {
            window,
            timer_type,
            timer_id,
        };
        let event = TimerEvent::window(payload, Timer::new(duration));
        event.schedule();
    }

    pub fn post_timer_event(payload: WindowTimerEvent) {
        match payload.timer_type {
            WindowTimerType::UserDefined => {
                let _result = payload.window.post(WindowMessage::Timer(payload.timer_id));
            }
        }
    }

    fn _contains(test: &RwLock<Option<WindowHandle>>, value: &WindowHandle) -> bool {
        let test = test.read().unwrap();
        if let Some(test) = test.as_ref() {
            *test == *value
        } else {
            false
        }
    }

    rwlock_helper! {
        fn captured(self) -> Option<WindowHandle>;

        fn active(self) -> Option<WindowHandle>;

        fn entered(self) -> Option<WindowHandle>;
    }
}

pub enum WindowTimerType {
    UserDefined,
}

pub struct WindowTimerEvent {
    timer_type: WindowTimerType,
    window: WindowHandle,
    timer_id: usize,
}

my_bitflags! {

    pub struct WindowManagerAttributes: usize {
        const EVENT             = 0x0000_0001;
        const NEEDS_REDRAW      = 0x0000_0002;

        const EVENT_MOUSE_MOVE  = 0x0000_0100;
        const EVENT_MOUSE_SHOW  = 0x0000_0200;
        const HW_CURSOR         = 0x0000_0400;
        const POINTER_HIDE_TEMP = 0x0000_2000;
        const POINTER_VISIBLE   = 0x0000_4000;
        const POINTER_ENABLED   = 0x0000_8000;

        const MOVING            = 0x0001_0000;
        const CLOSE_DOWN        = 0x0002_0000;
        const BACK_DOWN         = 0x0004_0000;
    }
}

impl Default for WindowManagerAttributes {
    #[inline]
    fn default() -> Self {
        Self::POINTER_VISIBLE
    }
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ViewActionState {
    Normal,
    Hover,
    Pressed,
    Disabled,
}

impl Default for ViewActionState {
    #[inline]
    fn default() -> Self {
        Self::Normal
    }
}

/// Raw implementation of the window
#[allow(dead_code)]
struct RawWindow {
    /// Refer to the self owned handle
    handle: WindowHandle,
    pid: ProcessId,

    // Properties
    attributes: AtomicFlags<WindowAttributes>,
    style: AtomicFlags<WindowStyle>,
    level: WindowLevel,

    // Placement and Size
    frame: Rect,
    content_insets: EdgeInsets,

    // Appearances
    bg_color: Color,
    accent_color: Color,
    active_title_color: Color,
    inactive_title_color: Color,
    bitmap: UnsafeCell<OwnedBitmap>,
    shadow_bitmap: Option<UnsafeCell<OperationalBitmap>>,
    back_buffer: UnsafeCell<OwnedBitmap32>,

    /// Window Title
    title: String,
    close_button_state: ViewActionState,
    back_button_state: ViewActionState,

    // Messages and Events
    waker: AtomicWaker,
    sem: Semaphore,
    queue: Option<ConcurrentFifo<WindowMessage>>,
}

my_bitflags! {
    pub struct WindowStyle: usize {
        const BORDER            = 0b0000_0000_0000_0001;
        const THIN_FRAME        = 0b0000_0000_0000_0010;
        const TITLE             = 0b0000_0000_0000_0100;
        const CLOSE_BUTTON      = 0b0000_0000_0000_1000;

        const OPAQUE_CONTENT    = 0b0000_0000_0001_0000;
        const OPAQUE            = 0b0000_0000_0010_0000;
        const NO_SHADOW         = 0b0000_0000_0100_0000;
        const FLOATING          = 0b0000_0000_1000_0000;

        const DARK_MODE         = 0b0000_0001_0000_0000;
        const DARK_BORDER       = 0b0000_0010_0000_0000;
        const DARK_TITLE        = 0b0000_0100_0000_0000;
        const DARK_ACTIVE       = 0b0000_1000_0000_0000;

        const PINCHABLE         = 0b0001_0000_0000_0000;
        const FULLSCREEN        = 0b0010_0000_0000_0000;
        const SUSPENDED         = 0b1000_0000_0000_0000;
    }
}

impl Default for WindowStyle {
    #[inline]
    fn default() -> Self {
        Self::DEFAULT
    }
}

impl WindowStyle {
    pub const DEFAULT: Self = Self::from_bits_retain(
        Self::BORDER.bits() | Self::TITLE.bits() | Self::CLOSE_BUTTON.bits(),
    );

    fn as_content_insets(self) -> EdgeInsets {
        let insets = if self.contains(Self::BORDER) {
            if self.contains(Self::THIN_FRAME) {
                if self.contains(Self::TITLE) {
                    EdgeInsets::new(
                        (WINDOW_BORDER_WIDTH + WINDOW_TITLE_HEIGHT + WINDOW_TITLE_BORDER) as i32,
                        (WINDOW_BORDER_WIDTH) as i32,
                        (WINDOW_BORDER_WIDTH) as i32,
                        (WINDOW_BORDER_WIDTH) as i32,
                    )
                } else {
                    EdgeInsets::padding_each(WINDOW_BORDER_WIDTH as i32)
                }
            } else {
                if self.contains(Self::TITLE) {
                    EdgeInsets::new(
                        (WINDOW_THICK_BORDER_WIDTH_V + WINDOW_TITLE_HEIGHT + WINDOW_TITLE_BORDER)
                            as i32,
                        (WINDOW_THICK_BORDER_WIDTH_H) as i32,
                        (WINDOW_THICK_BORDER_WIDTH_V) as i32,
                        (WINDOW_THICK_BORDER_WIDTH_H) as i32,
                    )
                } else {
                    EdgeInsets::new(
                        (WINDOW_THICK_BORDER_WIDTH_V) as i32,
                        (WINDOW_THICK_BORDER_WIDTH_H) as i32,
                        (WINDOW_THICK_BORDER_WIDTH_V) as i32,
                        (WINDOW_THICK_BORDER_WIDTH_H) as i32,
                    )
                }
            }
        } else {
            EdgeInsets::default()
        };
        insets
    }
}

my_bitflags! {
    pub struct WindowAttributes: usize {
        const NEEDS_REDRAW  = 0b0000_0001;
        const VISIBLE       = 0b0000_0010;
    }
}

impl RawWindow {
    #[inline]
    fn actual_bounds(&self) -> Rect {
        self.frame.bounds()
    }

    #[inline]
    fn visible_frame(&self) -> Rect {
        self.frame
    }

    #[inline]
    fn shadow_frame(&self) -> Rect {
        if self.style.contains(WindowStyle::NO_SHADOW) {
            self.frame
        } else {
            self.frame + EdgeInsets::padding_each(WINDOW_SHADOW_PADDING as i32)
        }
    }

    fn show(&mut self) {
        self.draw_frame();
        self.update_shadow();
        WindowManager::add_hierarchy(self.handle.clone());

        let frame = self.shadow_frame();
        self.draw_outer_to_screen(frame.origin().into(), frame.bounds(), false);
    }

    fn hide(&self) {
        let shared = WindowManager::shared();
        let frame = self.shadow_frame();
        let next_active = if WindowManager::_contains(&shared.active, &self.handle) {
            let window_orders = shared.window_orders.read().unwrap();
            window_orders
                .iter()
                .position(|v| *v == self.handle)
                .and_then(|v| window_orders.get(v - 1))
                .map(|v| v.clone())
        } else {
            None
        };
        {
            let mut captured_mut = shared.captured.write().unwrap();
            if let Some(captured) = captured_mut.as_ref() {
                if *captured == self.handle {
                    *captured_mut = None;
                }
            }
        }
        WindowManager::remove_hierarchy(self.handle.clone());
        WindowManager::invalidate_screen(frame);
        if next_active.is_some() {
            WindowManager::make_active(next_active);
        }
    }

    #[inline]
    pub fn close(&self) {
        WindowManager::remove(self);
    }

    fn set_frame(&mut self, new_frame: Rect) {
        let old_frame = self.frame;
        if old_frame != new_frame {
            let old_frame = self.shadow_frame();
            self.frame = new_frame;
            if self.attributes.contains(WindowAttributes::VISIBLE) {
                self.draw_frame();

                let Ok(coords1) = Coordinates::from_rect(old_frame) else {
                    return;
                };
                let Ok(coords2) = Coordinates::from_rect(self.shadow_frame()) else {
                    return;
                };
                WindowManager::invalidate_screen(Rect::from(coords1.merged(coords2)));
            }
        }
    }

    fn test_frame(&self, position: Point, frame: Rect) -> bool {
        let mut frame = frame;
        frame.origin += Point::from(self.frame.origin());
        frame.contains(position)
    }

    fn draw_inner_to_screen(&self, rect: Rect) {
        let Ok(coords) = Coordinates::from_rect(rect) else {
            return;
        };
        let bounds = self.frame.bounds();

        let is_opaque = self.style.contains(WindowStyle::OPAQUE)
            || self.style.contains(WindowStyle::OPAQUE_CONTENT)
                && bounds.insets_by(self.content_insets).contains(rect);

        let shared = WindowManager::shared();
        let is_direct = if is_opaque {
            let window_orders = shared.window_orders.read().unwrap();
            let first_index = 1 + window_orders
                .iter()
                .position(|v| *v == self.handle)
                .unwrap_or(0);
            let screen_rect = rect + Point::from(self.frame.origin());

            let mut is_direct = true;
            for handle in window_orders[first_index..].iter() {
                let Some(window) = handle.get() else { continue };
                if screen_rect.overlaps(window.shadow_frame()) {
                    is_direct = false;
                    break;
                }
            }
            is_direct
        } else {
            false
        };
        if is_direct {
            let offset = self.frame.origin();

            if let Some(screen) = System::main_screen() {
                screen.blt(
                    self.bitmap32().as_const(),
                    offset + coords.left_top(),
                    coords.into(),
                );
            }
        } else {
            let Ok(inner_coords) = Coordinates::from_rect(bounds) else {
                return;
            };
            let frame_origin = self.frame.origin();
            let offset = self.shadow_frame().origin();
            let rect = Rect::from(coords.trimmed(inner_coords)) + (frame_origin - offset);
            self.draw_outer_to_screen(offset, rect, is_opaque);
        }
    }

    fn draw_outer_to_screen(&self, offset: Point, rect: Rect, is_opaque: bool) {
        let screen_rect = rect + offset;
        let back_buffer = unsafe { &mut *self.back_buffer.get() };
        let back_buffer = back_buffer.as_mut();
        if self.draw_into(back_buffer, offset, screen_rect, is_opaque) {
            if let Some(screen) = System::main_screen() {
                screen.blt(back_buffer.as_const(), rect.origin() + offset, rect);
            }
        }
    }

    fn draw_into(
        &self,
        target_bitmap: &mut BitmapRefMut32,
        offset: Point,
        frame1: Rect,
        is_opaque: bool,
    ) -> bool {
        let Ok(coords1) = Coordinates::from_rect(frame1) else {
            return false;
        };

        let window_orders = WindowManager::shared().window_orders.read().unwrap();

        let first_index = if is_opaque {
            window_orders
                .iter()
                .position(|v| *v == self.handle)
                .unwrap_or(0)
        } else {
            0
        };

        for handle in window_orders[first_index..].iter() {
            let window = handle.as_ref();
            let frame2 = window.shadow_frame();
            let Ok(coords2) = Coordinates::from_rect(frame2) else {
                continue;
            };
            if frame2.overlaps(frame1) {
                let adjust_point = window.frame.origin() - coords2.left_top();
                let blt_origin =
                    Point::new(coords1.left.max(coords2.left), coords1.top.max(coords2.top))
                        - offset;
                let target_rect = Rect::new(
                    (coords1.left - coords2.left).max(0),
                    (coords1.top - coords2.top).max(0),
                    (coords1.right.min(coords2.right) - coords1.left.max(coords2.left)) as u32,
                    (coords1.bottom.min(coords2.bottom) - coords1.top.max(coords2.top)) as u32,
                );

                let bitmap = window.bitmap32();
                let blt_rect = target_rect - adjust_point;
                if window.style.contains(WindowStyle::OPAQUE)
                    || self.handle == window.handle && is_opaque
                {
                    target_bitmap.blt(bitmap.as_const(), blt_origin, blt_rect);
                } else {
                    target_bitmap.blt_blend(
                        bitmap.as_const(),
                        blt_origin,
                        blt_rect,
                        Alpha8::OPAQUE,
                    );
                }

                if !window
                    .frame
                    .insets_by(window.content_insets)
                    .contains(frame1)
                {
                    if let Some(shadow) = window.shadow_bitmap() {
                        shadow.blt_shadow(target_bitmap, blt_origin, target_rect);
                    }
                }
            }
        }

        true
    }

    fn set_bg_color(&mut self, color: Color) {
        self.bg_color = color;
        self.style.set(
            WindowStyle::DARK_BORDER,
            color.brightness().unwrap_or(255) < 128,
        );
        let bitmap = self.bitmap();
        bitmap.fill_rect(bitmap.bounds(), color.into());
        self.draw_frame();
        self.set_needs_display();
    }

    fn title_frame(&self) -> Rect {
        if self.style.contains(WindowStyle::TITLE) {
            Rect::new(
                WINDOW_BORDER_WIDTH as i32,
                WINDOW_BORDER_WIDTH as i32,
                self.frame.width() - WINDOW_BORDER_WIDTH * 2,
                WINDOW_TITLE_HEIGHT,
            )
        } else {
            Rect::default()
        }
    }

    fn close_button_frame(&self) -> Rect {
        let shared = WindowManager::shared();
        let rect = self.title_frame();
        let window_button_width = shared.resources.window_button_width;
        Rect::new(
            rect.max_x() - window_button_width as i32 - WINDOW_CORNER_RADIUS as i32,
            rect.min_y(),
            window_button_width,
            rect.height(),
        )
    }

    fn back_button_frame(&self) -> Rect {
        let shared = WindowManager::shared();
        let rect = self.title_frame();
        let window_button_width = shared.resources.window_button_width;
        Rect::new(
            WINDOW_CORNER_RADIUS as i32,
            rect.min_y(),
            window_button_width,
            rect.height(),
        )
    }

    #[inline]
    fn is_active(&self) -> bool {
        let shared = WindowManager::shared();
        WindowManager::_contains(&shared.active, &self.handle)
    }

    fn refresh_title(&mut self) {
        self.draw_frame();
        if self.style.contains(WindowStyle::TITLE) {
            self.invalidate_rect(self.title_frame());
        }
    }

    fn draw_frame(&mut self) {
        let bitmap = self.bitmap();
        let is_thin = self.style.contains(WindowStyle::THIN_FRAME);
        let is_dark = self.style.contains(WindowStyle::DARK_BORDER);

        if self.style.contains(WindowStyle::TITLE) {
            let shared = WindowManager::shared();
            let padding = 8;
            let left = padding;
            let right = padding;

            let frame = self.visible_frame();
            if WINDOW_TITLE_BORDER > 0 {
                bitmap.fill_rect(
                    Rect::new(
                        0,
                        WINDOW_BORDER_WIDTH as i32 + WINDOW_TITLE_HEIGHT as i32,
                        frame.width(),
                        WINDOW_TITLE_BORDER,
                    ),
                    if is_dark {
                        Theme::shared().window_default_border_dark()
                    } else {
                        Theme::shared().window_default_border_light()
                    },
                );
            }
            bitmap.fill_rect(self.title_frame(), self.title_background());
            self.draw_close_button();
            self.draw_back_button();

            bitmap
                .sub_image(self.title_frame())
                .map(|mut bitmap| {
                    let bitmap = &mut bitmap;
                    let rect = bitmap.bounds();

                    AttributedString::new()
                        .font(&shared.resources.title_font)
                        .color(self.title_foreground())
                        .center()
                        .shadow(self.title_shadow_color(), Point::new(1, 1))
                        .text(self.title())
                        .draw_text(
                            bitmap,
                            rect.insets_by(EdgeInsets::new(0, left, 0, right)),
                            1,
                        );
                })
                .unwrap();
        }

        if self.style.contains(WindowStyle::BORDER) {
            if is_thin {
                if WINDOW_BORDER_WIDTH > 0 {
                    let rect = Rect::from(bitmap.size());
                    bitmap.draw_rect(
                        rect,
                        if is_dark {
                            Theme::shared().window_default_border_dark()
                        } else {
                            Theme::shared().window_default_border_light()
                        },
                    );
                }
            } else {
                let rect = Rect::from(bitmap.size());
                let border_color = if is_dark {
                    Theme::shared().window_default_border_dark()
                } else {
                    Theme::shared().window_default_border_light()
                };

                bitmap.draw_round_rect(rect, WINDOW_CORNER_RADIUS, border_color);

                if let Ok(coord) = Coordinates::from_rect(rect) {
                    let lt = coord.left_top();
                    let rt = coord.right_top();
                    let lb = coord.left_bottom();
                    let rb = coord.right_bottom();

                    for (i, w) in CORNER_MASK.iter().enumerate() {
                        let y = i as i32;
                        let w = *w as i32;
                        for origin in [
                            lt + Point::new(0, y),
                            rt + Point::new(-w, y),
                            lb + Point::new(0, -y - 1),
                            rb + Point::new(-w, -y - 1),
                        ] {
                            bitmap.draw_hline(origin, w as u32, Color::TRANSPARENT);
                        }
                    }
                }
            }
        }
    }

    #[inline]
    fn title_background(&self) -> Color {
        if self.is_active() {
            self.active_title_color
        } else {
            self.inactive_title_color
        }
    }

    #[inline]
    fn title_foreground(&self) -> Color {
        if self.is_active() {
            if self.style.contains(WindowStyle::DARK_ACTIVE) {
                Theme::shared().window_title_active_foreground_dark()
            } else {
                Theme::shared().window_title_active_foreground()
            }
        } else {
            if self.style.contains(WindowStyle::DARK_TITLE) {
                Theme::shared().window_title_inactive_foreground_dark()
            } else {
                Theme::shared().window_title_inactive_foreground()
            }
        }
    }

    #[inline]
    fn title_shadow_color(&self) -> Color {
        if self.is_active() {
            if self.style.contains(WindowStyle::DARK_ACTIVE) {
                Theme::shared().window_title_active_shadow_dark()
            } else {
                Theme::shared().window_title_active_shadow()
            }
        } else {
            Color::TRANSPARENT
        }
    }

    fn draw_close_button(&self) {
        if !self.style.contains(WindowStyle::TITLE) {
            return;
        }
        let bitmap = self.bitmap();
        let shared = WindowManager::shared();
        let state = self.close_button_state;
        let button_frame = self.close_button_frame();
        let is_active = self.is_active() && state != ViewActionState::Disabled;

        let background = match state {
            ViewActionState::Pressed => Theme::shared().window_title_close_active_background(),
            _ => self.title_background(),
        };
        let foreground = match state {
            ViewActionState::Pressed => Theme::shared().window_title_close_active_foreground(),
            _ => {
                if is_active {
                    if self.style.contains(WindowStyle::DARK_ACTIVE) {
                        Theme::shared().window_title_close_foreground_dark()
                    } else {
                        Theme::shared().window_title_close_foreground()
                    }
                } else {
                    if self.style.contains(WindowStyle::DARK_TITLE) {
                        Theme::shared().window_title_inactive_foreground_dark()
                    } else {
                        Theme::shared().window_title_inactive_foreground()
                    }
                }
            }
        }
        .into_true_color();

        bitmap.fill_rect(button_frame, background);

        let button = &shared.resources.close_button;
        let origin = Point::new(
            button_frame.min_x() + ((button_frame.width() - button.width()) / 2) as i32,
            button_frame.min_y() + ((button_frame.height() - button.height()) / 2) as i32,
        );
        button.draw_to(bitmap, origin, button.bounds(), foreground.into());
    }

    fn draw_back_button(&self) {
        if !self.style.contains(WindowStyle::TITLE) {
            return;
        }
        let bitmap = self.bitmap();
        let state = match self.back_button_state {
            ViewActionState::Disabled => return,
            other => other,
        };
        let shared = WindowManager::shared();
        let button_frame = self.back_button_frame();
        let is_active = self.is_active();

        let background = match state {
            ViewActionState::Pressed => Theme::shared().window_title_close_active_background(),
            _ => self.title_background(),
        };
        let foreground = match state {
            ViewActionState::Pressed => Theme::shared().window_title_close_active_foreground(),
            _ => {
                if is_active {
                    if self.style.contains(WindowStyle::DARK_ACTIVE) {
                        Theme::shared().window_title_close_foreground_dark()
                    } else {
                        Theme::shared().window_title_close_foreground()
                    }
                } else {
                    self.title_foreground()
                }
            }
        }
        .into_true_color();

        bitmap.fill_rect(button_frame, background);

        let button = &shared.resources.back_button;
        let origin = Point::new(
            button_frame.min_x() + ((button_frame.width() - button.width()) / 2) as i32,
            button_frame.min_y() + ((button_frame.height() - button.height()) / 2) as i32,
        );
        button.draw_to(bitmap, origin, button.bounds(), foreground.into());
    }

    #[inline]
    pub fn set_needs_display(&self) {
        match self.handle.post(WindowMessage::Draw) {
            Ok(_) => {}
            Err(_) => {
                WindowManager::invalidate_screen(self.shadow_frame());
            }
        }
    }

    fn invalidate_rect(&mut self, rect: Rect) {
        if self.attributes.contains(WindowAttributes::VISIBLE) {
            self.draw_inner_to_screen(rect);
        }
    }

    #[inline]
    fn set_title(&mut self, title: &str) {
        self.title = title.to_owned();
        self.draw_frame();
        self.invalidate_rect(self.title_frame());
    }

    #[inline]
    fn set_close_state(&mut self, state: ViewActionState) {
        if self.close_button_state != state {
            self.close_button_state = state;
            self.update_close_button();
        }
    }

    #[inline]
    fn set_back_state(&mut self, state: ViewActionState) {
        if self.back_button_state != state {
            self.back_button_state = state;
            if state == ViewActionState::Disabled {
                self.draw_frame();
                self.invalidate_rect(self.title_frame());
            } else {
                self.update_back_button();
            }
        }
    }

    #[inline]
    fn update_close_button(&mut self) {
        self.draw_close_button();
        self.invalidate_rect(self.close_button_frame());
    }

    #[inline]
    fn update_back_button(&mut self) {
        self.draw_back_button();
        self.invalidate_rect(self.back_button_frame());
    }

    #[inline]
    fn shadow_bitmap<'a>(&'a self) -> Option<&'a mut OperationalBitmap> {
        self.shadow_bitmap
            .as_ref()
            .map(|v| unsafe { &mut *v.get() })
    }

    fn update_shadow(&self) {
        let bitmap = self.bitmap();
        let Some(shadow) = self.shadow_bitmap() else {
            return;
        };

        shadow.reset();

        let content_rect = Rect::from(self.frame.size());
        let origin = Point::new(
            WINDOW_SHADOW_PADDING as i32 - SHADOW_RADIUS as i32,
            WINDOW_SHADOW_PADDING as i32 - SHADOW_RADIUS as i32,
        ) + SHADOW_OFFSET;
        shadow.blt_from(bitmap, origin, content_rect, |a, _| {
            let a = a.into_true_color().opacity();
            a.saturating_add(a).as_u8()
        });

        shadow.blur(SHADOW_RADIUS, SHADOW_LEVEL);

        shadow.blt_from(
            bitmap,
            Point::new(WINDOW_SHADOW_PADDING as i32, WINDOW_SHADOW_PADDING as i32),
            bitmap.bounds(),
            |a, b| {
                if a.into_true_color().opacity().as_u8() >= b {
                    0
                } else {
                    b
                }
            },
        );
    }

    #[inline]
    fn bitmap<'a>(&self) -> &'a mut BitmapRefMut<'a> {
        unsafe { &mut *self.bitmap.get() }.as_mut()
    }

    #[inline]
    fn bitmap32<'a>(&self) -> &'a mut BitmapRefMut32<'a> {
        match self.bitmap() {
            BitmapRefMut::Indexed(_) => unreachable!(),
            BitmapRefMut::Argb32(ref mut v) => v,
        }
    }

    #[inline]
    fn title(&self) -> &str {
        self.title.as_str()
    }

    fn draw_in_rect<'a, F>(&'a self, rect: Rect, f: F) -> Result<(), WindowDrawingError>
    where
        F: 'a + FnOnce(&mut BitmapRefMut) -> (),
    {
        let bitmap = self.bitmap();
        let bounds = self.frame.bounds().insets_by(self.content_insets);
        let origin = Point::new(rect.min_x().max(0), rect.min_y().max(0));
        let Ok(coords) = Coordinates::from_rect(Rect::new(
            origin.x + bounds.min_x(),
            origin.y + bounds.min_y(),
            rect.width().min(bounds.width() - origin.x as u32),
            rect.height().min(bounds.height() - origin.y as u32),
        )) else {
            return Err(WindowDrawingError::InconsistentCoordinates);
        };
        if coords.left > coords.right || coords.top > coords.bottom {
            return Err(WindowDrawingError::InconsistentCoordinates);
        }

        let rect = coords.into();
        bitmap
            .sub_image(rect)
            .map(|mut bitmap| f(&mut bitmap))
            .ok_or(WindowDrawingError::InconsistentCoordinates)
    }
}

#[repr(transparent)]
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct WindowLevel(pub u8);

impl WindowLevel {
    /// Root window (desktop)
    pub const ROOT: WindowLevel = WindowLevel(0);
    /// Items on the desktop
    pub const DESKTOP_ITEMS: WindowLevel = WindowLevel(1);
    /// Normal window
    pub const NORMAL: WindowLevel = WindowLevel(32);
    /// Floating window
    pub const FLOATING: WindowLevel = WindowLevel(64);
    pub const POPUP_BARRIER_BG: WindowLevel = WindowLevel(96);
    /// Popup barrier
    pub const POPUP_BARRIER: WindowLevel = WindowLevel(97);
    /// Popup window
    pub const POPUP: WindowLevel = WindowLevel(98);
    /// The mouse pointer, which is also the foremost window.
    pub const POINTER: WindowLevel = WindowLevel(127);
}

pub struct RawWindowBuilder {
    frame: Rect,
    style: WindowStyle,
    options: u32,
    level: WindowLevel,
    bg_color: Color,
    active_title_color: Option<Color>,
    inactive_title_color: Option<Color>,

    queue_size: usize,
    bitmap_strategy: BitmapStrategy,
}

impl RawWindowBuilder {
    #[inline]
    pub fn new() -> Self {
        Self {
            frame: Rect::new(i32::MIN, i32::MIN, 300, 300),
            level: WindowLevel::NORMAL,
            style: WindowStyle::default(),
            options: 0,
            bg_color: Theme::shared().window_default_background(),
            active_title_color: None,
            inactive_title_color: None,
            queue_size: 100,
            bitmap_strategy: BitmapStrategy::default(),
        }
    }

    pub fn build(self, title: &str) -> WindowHandle {
        let window = self.build_inner(title);
        let handle = window.handle.clone();
        let style = window.style.value();
        WindowManager::add(window);
        if !style.contains(WindowStyle::SUSPENDED) {
            handle.make_active();
        }
        handle
    }

    fn build_inner<'a>(mut self, title: &str) -> RawWindow {
        let window_options = self.options;
        if (window_options & megos::window::THIN_FRAME) != 0 {
            self.style.insert(WindowStyle::THIN_FRAME);
        }
        if (window_options & megos::window::OPAQUE_CONTENT) != 0 {
            self.style.insert(WindowStyle::OPAQUE_CONTENT);
        }
        if (window_options & megos::window::USE_BITMAP32) != 0 {
            self.bitmap_strategy = BitmapStrategy::Expressive;
        }
        if (window_options & megos::window::FULLSCREEN) != 0 {
            self.style.insert(WindowStyle::FULLSCREEN);
        }
        if self.style.contains(WindowStyle::THIN_FRAME) {
            self.style.insert(WindowStyle::BORDER);
        }

        // // Here you can experiment with forcing window attributes.
        // if self.style.contains(WindowStyle::BORDER) {
        //     self.style.insert(WindowStyle::THIN_FRAME);
        // }
        // self.style.insert(WindowStyle::NO_SHADOW);
        // self.style.insert(WindowStyle::DARK_MODE);

        if self.style.contains(WindowStyle::FLOATING) && self.level <= WindowLevel::NORMAL {
            self.level = WindowLevel::FLOATING;
        }

        let screen_bounds = WindowManager::user_screen_bounds();
        let content_insets = self.style.as_content_insets();
        let frame = if self.style.contains(WindowStyle::FULLSCREEN) {
            if self.level >= WindowLevel::FLOATING {
                WindowManager::main_screen_bounds()
            } else {
                WindowManager::user_screen_bounds()
            }
        } else {
            let mut frame = self.frame;
            frame.size += content_insets;
            if frame.min_x() == i32::MIN {
                frame.origin.x = (screen_bounds.max_x() - frame.width() as i32) / 2;
            } else if frame.min_x() < 0 {
                frame.origin.x +=
                    screen_bounds.max_x() - (content_insets.left + content_insets.right);
            }
            if frame.min_y() == i32::MIN {
                frame.origin.y = screen_bounds
                    .min_y()
                    .max((screen_bounds.max_y() - frame.height() as i32) / 2);
            } else if frame.min_y() < 0 {
                frame.origin.y +=
                    screen_bounds.max_y() - (content_insets.top + content_insets.bottom);
            }
            frame
        };

        let attributes = if self.level == WindowLevel::ROOT {
            AtomicFlags::new(WindowAttributes::VISIBLE)
        } else {
            AtomicFlags::empty()
        };

        let bg_color = self.bg_color;

        let is_dark_mode = self.style.contains(WindowStyle::DARK_MODE);

        self.style.set(
            WindowStyle::DARK_BORDER,
            is_dark_mode,
            // bg_color.brightness().unwrap_or(255) < 128,
        );

        let accent_color = Theme::shared().window_default_accent();
        let active_title_color = self.active_title_color.unwrap_or(if is_dark_mode {
            Theme::shared().window_title_active_background_dark()
        } else {
            Theme::shared().window_title_active_background()
        });
        let inactive_title_color = self.inactive_title_color.unwrap_or(if is_dark_mode {
            Theme::shared().window_title_inactive_background_dark()
        } else {
            Theme::shared().window_title_inactive_background()
        });
        self.style.set(
            WindowStyle::DARK_ACTIVE,
            active_title_color.brightness().unwrap_or(255) < 192,
        );
        self.style.set(
            WindowStyle::DARK_TITLE,
            inactive_title_color.brightness().unwrap_or(255) < 128,
        );

        let queue = match self.queue_size {
            0 => None,
            _ => Some(ConcurrentFifo::with_capacity(self.queue_size)),
        };

        let bitmap = UnsafeCell::new(OwnedBitmap::Argb32(OwnedBitmap32::new(
            frame.size(),
            bg_color.into(),
        )));

        let shadow_bitmap = if self.style.contains(WindowStyle::NO_SHADOW) {
            None
        } else {
            let mut shadow = OperationalBitmap::new(
                frame.size() + Size::new(WINDOW_SHADOW_PADDING * 2, WINDOW_SHADOW_PADDING * 2),
            );
            shadow.reset();
            Some(UnsafeCell::new(shadow))
        };

        let close_button_state = if self.style.contains(WindowStyle::CLOSE_BUTTON) {
            Default::default()
        } else {
            ViewActionState::Disabled
        };

        let back_buffer = if let Some(ref shadow_bitmap) = shadow_bitmap {
            let shadow_bitmap = unsafe { &*shadow_bitmap.get() };
            UnsafeCell::new(OwnedBitmap32::new(
                shadow_bitmap.size(),
                TrueColor::TRANSPARENT,
            ))
        } else {
            UnsafeCell::new(OwnedBitmap32::new(frame.size(), TrueColor::TRANSPARENT))
        };

        let handle = WindowManager::next_window_handle();

        RawWindow {
            handle,
            frame,
            content_insets,
            style: AtomicFlags::new(self.style),
            level: self.level,
            bg_color,
            accent_color,
            active_title_color,
            inactive_title_color,
            bitmap,
            shadow_bitmap,
            back_buffer,
            title: title.to_owned(),
            close_button_state,
            back_button_state: ViewActionState::Disabled,
            attributes,
            waker: AtomicWaker::new(),
            sem: Semaphore::new(0),
            queue,
            pid: Scheduler::current_pid(),
        }
    }

    #[inline]
    pub const fn style(mut self, style: WindowStyle) -> Self {
        self.style = style;
        self
    }

    #[inline]
    pub const fn style_add(mut self, style: WindowStyle) -> Self {
        self.style = WindowStyle::from_bits_retain(self.style.bits() | style.bits());
        self
    }

    #[inline]
    pub const fn style_sub(mut self, style: WindowStyle) -> Self {
        self.style = WindowStyle::from_bits_retain(self.style.bits() & !style.bits());
        self
    }

    #[inline]
    pub const fn level(mut self, level: WindowLevel) -> Self {
        self.level = level;
        self
    }

    #[inline]
    pub const fn frame(mut self, frame: Rect) -> Self {
        self.frame = frame;
        self
    }

    #[inline]
    pub const fn center(mut self) -> Self {
        self.frame.origin = Point::new(i32::MIN, i32::MIN);
        self
    }

    #[inline]
    pub const fn origin(mut self, origin: Point) -> Self {
        self.frame.origin = origin;
        self
    }

    #[inline]
    pub const fn size(mut self, size: Size) -> Self {
        self.frame.size = size;
        self
    }

    #[inline]
    pub const fn bg_color(mut self, bg_color: Color) -> Self {
        self.bg_color = bg_color;
        self
    }

    #[inline]
    pub const fn active_title_color(mut self, active_title_color: Color) -> Self {
        self.active_title_color = Some(active_title_color);
        self
    }

    #[inline]
    pub const fn inactive_title_color(mut self, inactive_title_color: Color) -> Self {
        self.inactive_title_color = Some(inactive_title_color);
        self
    }

    #[inline]
    const fn without_message_queue(mut self) -> Self {
        self.queue_size = 0;
        self
    }

    #[inline]
    const fn bitmap_strategy(mut self, bitmap_strategy: BitmapStrategy) -> Self {
        self.bitmap_strategy = bitmap_strategy;
        self
    }

    /// Sets the window's content bitmap to ARGB32 format.
    #[inline]
    pub const fn bitmap_argb32(mut self) -> Self {
        self.options |= megos::window::USE_BITMAP32;
        self
    }

    /// Makes the border of the window a thin border.
    #[inline]
    pub const fn thin_frame(mut self) -> Self {
        self.options |= megos::window::THIN_FRAME;
        self
    }

    /// Content is opaque
    #[inline]
    pub const fn opaque(mut self) -> Self {
        self.options |= megos::window::OPAQUE_CONTENT;
        self
    }

    #[inline]
    pub const fn fullscreen(mut self) -> Self {
        self.options |= megos::window::FULLSCREEN;
        self
    }

    #[inline]
    pub const fn with_options(mut self, options: u32) -> Self {
        self.options = options;
        self
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BitmapStrategy {
    NonBitmap,
    Native,
    Compact,
    Expressive,
}

impl Default for BitmapStrategy {
    fn default() -> Self {
        Self::Compact
    }
}

#[repr(transparent)]
struct WindowRef(Arc<UnsafeCell<RawWindow>>);

impl Deref for WindowRef {
    type Target = RawWindow;

    fn deref(&self) -> &Self::Target {
        unsafe { &*self.0.as_ref().get() }
    }
}

#[repr(transparent)]
#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct WindowHandle(NonZeroUsize);

impl WindowHandle {
    #[inline]
    pub const fn new(val: usize) -> Option<Self> {
        match NonZeroUsize::new(val) {
            Some(v) => Some(WindowHandle(v)),
            None => None,
        }
    }

    #[inline]
    pub const fn as_usize(&self) -> usize {
        self.0.get()
    }

    #[inline]
    pub fn validate(&self) -> Option<Self> {
        self.get().map(|v| v.handle.clone())
    }

    #[inline]
    #[track_caller]
    fn get<'a>(&self) -> Option<WindowRef> {
        WindowManager::shared()._get(self)
    }

    #[inline]
    #[track_caller]
    fn as_ref<'a>(&self) -> WindowRef {
        self.get().unwrap()
    }

    #[inline]
    fn update_opt<F, R>(&self, f: F) -> Option<R>
    where
        F: FnOnce(&mut RawWindow) -> R,
    {
        WindowManager::shared().update(self, f)
    }

    #[inline]
    #[track_caller]
    fn update<F, R>(&self, f: F) -> R
    where
        F: FnOnce(&mut RawWindow) -> R,
    {
        self.update_opt(f).unwrap()
    }

    #[inline]
    pub fn is_active(&self) -> bool {
        self.get().map(|v| v.is_active()).unwrap_or(false)
    }

    #[inline]
    pub fn set_title(&self, title: &str) {
        self.update(|window| {
            window.set_title(title);
        });
    }

    #[inline]
    pub fn set_bg_color(&self, color: Color) {
        self.update(|window| {
            window.set_bg_color(color);
        });
    }

    #[inline]
    pub fn bg_color(&self) -> Color {
        self.as_ref().bg_color
    }

    #[inline]
    pub fn active_title_color(&self) -> Color {
        self.as_ref().active_title_color
    }

    #[inline]
    pub fn inactive_title_color(&self) -> Color {
        self.as_ref().inactive_title_color
    }

    #[inline]
    pub fn frame(&self) -> Rect {
        self.as_ref().visible_frame()
    }

    #[inline]
    pub fn set_frame(&self, rect: Rect) {
        self.update(|window| {
            window.set_frame(rect);
        });
    }

    #[inline]
    pub fn content_insets(&self) -> EdgeInsets {
        self.as_ref().content_insets
    }

    #[inline]
    pub fn content_rect(&self) -> Rect {
        let window = self.as_ref();
        window.frame.bounds().insets_by(window.content_insets)
    }

    #[inline]
    pub fn content_size(&self) -> Size {
        self.content_rect().size()
    }

    #[inline]
    pub fn move_by(&self, movement: Point) {
        self.set_frame(self.frame() + movement);
    }

    #[inline]
    pub fn move_to(&self, new_origin: Point) {
        let mut new_rect = self.frame();
        new_rect.origin = new_origin;
        self.set_frame(new_rect);
    }

    #[inline]
    pub fn resize_to(&self, new_size: Size) {
        let mut new_rect = self.frame();
        new_rect.size = new_size;
        self.set_frame(new_rect);
    }

    #[inline]
    pub fn show(&self) {
        self.update(|window| window.show());
    }

    #[inline]
    pub fn hide(&self) {
        self.update(|window| window.hide());
    }

    #[inline]
    pub fn close(&self) {
        self.as_ref().close();
    }

    #[inline]
    pub fn is_visible(&self) -> bool {
        self.as_ref().attributes.contains(WindowAttributes::VISIBLE)
    }

    #[inline]
    pub fn make_active(&self) {
        WindowManager::make_active(Some(self.clone()));
    }

    #[inline]
    pub fn set_close_button_enabled(&self, enabled: bool) {
        self.update(|window| {
            if enabled {
                window.set_close_state(ViewActionState::Normal)
            } else {
                window.set_close_state(ViewActionState::Disabled)
            }
        });
    }

    #[inline]
    pub fn set_back_button_enabled(&self, enabled: bool) {
        self.update(|window| {
            if enabled {
                window.set_back_state(ViewActionState::Normal)
            } else {
                window.set_back_state(ViewActionState::Disabled)
            }
        });
    }

    #[inline]
    pub fn invalidate_rect(&self, rect: Rect) {
        let _ = self.update_opt(|window| {
            let mut frame = rect;
            frame.origin.x += window.content_insets.left;
            frame.origin.y += window.content_insets.top;
            window.invalidate_rect(frame);
        });
    }

    #[inline]
    pub fn set_needs_display(&self) {
        self.as_ref().set_needs_display();
    }

    #[inline]
    pub fn draw<F>(&self, f: F)
    where
        F: FnOnce(&mut BitmapRefMut) -> (),
    {
        self.update(|window| {
            let rect = window.actual_bounds().insets_by(window.content_insets);
            match self.draw_in_rect(rect.size().into(), f) {
                Ok(_) | Err(WindowDrawingError::NoBitmap) => {
                    window.invalidate_rect(rect);
                }
                Err(_) => (),
            }
        })
    }

    pub fn draw_in_rect<F>(&self, rect: Rect, f: F) -> Result<(), WindowDrawingError>
    where
        F: FnOnce(&mut BitmapRefMut) -> (),
    {
        match self.get() {
            Some(v) => v.draw_in_rect(rect, f),
            None => Err(WindowDrawingError::NoWindow),
        }
    }

    /// Draws the contents of the window on the screen as a bitmap.
    pub fn draw_into(&self, target_bitmap: &mut BitmapRefMut32, rect: Rect) {
        let window = self.as_ref();
        window.draw_into(
            target_bitmap,
            Point::default(),
            rect + Point::from(window.frame.origin()),
            false,
        );
    }

    /// Post a window message.
    pub fn post(&self, message: WindowMessage) -> Result<(), WindowPostError> {
        let Some(window) = self.get() else {
            return Err(WindowPostError::NotFound);
        };
        if let Some(queue) = window.queue.as_ref() {
            match message {
                WindowMessage::Draw => {
                    window.attributes.insert(WindowAttributes::NEEDS_REDRAW);
                    window.waker.wake();
                    window.sem.signal();
                    Ok(())
                }
                _ => queue
                    .enqueue(message)
                    .map_err(|_| WindowPostError::Full)
                    .map(|_| {
                        window.waker.wake();
                        window.sem.signal();
                    }),
            }
        } else {
            Err(WindowPostError::NotFound)
        }
    }

    /// Read a window message from the message queue.
    pub fn read_message(&self) -> Option<WindowMessage> {
        let Some(window) = self.get() else {
            return None;
        };
        if let Some(queue) = window.queue.as_ref() {
            match queue.dequeue() {
                Some(v) => Some(v),
                _ => {
                    if window
                        .attributes
                        .fetch_reset(WindowAttributes::NEEDS_REDRAW)
                    {
                        Some(WindowMessage::Draw)
                    } else {
                        None
                    }
                }
            }
        } else {
            None
        }
    }

    /// Wait for window messages to be read.
    pub fn wait_message(&self) -> Option<WindowMessage> {
        loop {
            let Some(window) = self.get() else {
                return None;
            };
            match self.read_message() {
                Some(message) => return Some(message),
                None => window.sem.wait(),
            }
        }
    }

    /// Get the window message asynchronously.
    pub fn await_message(&self) -> Pin<Box<dyn Future<Output = Option<WindowMessage>>>> {
        Box::pin(WindowMessageConsumer {
            handle: self.clone(),
        })
    }

    /// Supports asynchronous reading of window messages.
    pub fn poll_message(&self, cx: &mut Context<'_>) -> Poll<Option<WindowMessage>> {
        let Some(window) = self.get() else {
            return Poll::Ready(None);
        };
        window.waker.register(cx.waker());
        match self.read_message().map(|message| {
            self.as_ref().waker.take();
            message
        }) {
            Some(v) => Poll::Ready(Some(v)),
            None => Poll::Pending,
        }
    }

    /// Process window messages that are not handled.
    pub fn handle_default_message(&self, message: WindowMessage) {
        match message {
            WindowMessage::Draw => self.draw(|_| {}),
            WindowMessage::Key(key) => {
                if let Some(c) = key.key_data().map(|v| v.into_char()) {
                    let _ = self.post(WindowMessage::Char(c));
                }
            }
            _ => {}
        }
    }

    ///
    pub fn refresh_if_needed(&self) {
        let Some(window) = self.get() else { return };
        if window
            .attributes
            .fetch_reset(WindowAttributes::NEEDS_REDRAW)
        {
            self.draw(|_| {})
        }
    }

    /// Create a timer associated with a window
    pub fn create_timer(&self, timer_id: usize, duration: Duration) {
        WindowManager::_create_timer(self, WindowTimerType::UserDefined, timer_id, duration);
    }
}

impl Clone for WindowHandle {
    #[inline]
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

struct WindowMessageConsumer {
    handle: WindowHandle,
}

impl Future for WindowMessageConsumer {
    type Output = Option<WindowMessage>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        self.handle.poll_message(cx)
    }
}

#[non_exhaustive]
#[derive(Debug, Copy, Clone)]
pub enum WindowDrawingError {
    NoBitmap,
    NoWindow,
    InconsistentCoordinates,
}

#[non_exhaustive]
#[derive(Debug, Copy, Clone)]
pub enum WindowPostError {
    NotFound,
    Full,
}

#[non_exhaustive]
#[derive(Debug, Copy, Clone)]
pub enum WindowMessage {
    /// Dummy message
    Nop,
    /// Requested to close the window
    Close,
    ///
    Back,
    /// Needs to be redrawn
    Draw,
    // Active
    Activated,
    Deactivated,
    /// Raw keyboard event
    Key(KeyEvent),
    /// Unicode converted keyboard event
    Char(char),
    // mouse events
    MouseMove(MouseEvent),
    MouseDown(MouseEvent),
    MouseUp(MouseEvent),
    MouseEnter(MouseEvent),
    MouseLeave(MouseEvent),
    /// Timer event
    Timer(usize),
    /// User Defined
    User(usize),
}

#[non_exhaustive]
#[derive(Debug)]
pub enum WindowSystemEvent {
    /// Raw Keyboard event
    Key(WindowHandle, KeyEvent),
}

pub struct AnimatedProp {
    start: f64,
    end: f64,
    start_time: Duration,
    duration: Duration,
}

impl AnimatedProp {
    #[inline]
    pub fn new(start: f64, end: f64, duration: Duration) -> Self {
        let start_time = Timer::monotonic();

        Self {
            start,
            end,
            start_time,
            duration,
        }
    }

    #[inline]
    pub const fn empty() -> Self {
        Self {
            start: 0.0,
            end: 0.0,
            start_time: Duration::from_millis(0),
            duration: Duration::from_millis(0),
        }
    }

    #[inline]
    pub fn is_alive(&self) -> bool {
        Timer::monotonic() < self.start_time + self.duration
    }

    pub fn progress(&self) -> f64 {
        let now = Timer::monotonic();
        let delta = now - self.start_time;
        let end_time = self.start_time + self.duration;

        if now < end_time {
            self.start
                + (self.end - self.start)
                    * (delta.as_micros() as f64 / self.duration.as_micros() as f64)
        } else {
            self.end
        }
    }
}
