use crate::{
    fs::*,
    io::{image::ImageLoader, tty::*},
    log::{EventManager, SimpleMessagePayload},
    mem::*,
    res::icon::IconManager,
    sync::fifo::{ConcurrentFifo, EventQueue},
    system::*,
    task::scheduler::*,
    ui::font::*,
    ui::terminal::Terminal,
    ui::text::*,
    ui::theme::Theme,
    ui::window::*,
    *,
};
use core::{
    fmt::Write,
    mem::{transmute, MaybeUninit},
    time::Duration,
};
use megstd::{
    drawing::{vertex::*, *},
    io::Read,
    string::*,
    Arc, String, Vec,
};

static IS_GUI_BOOT: bool = true;
static mut SHUTDOWN_COMMAND: MaybeUninit<EventQueue<ShutdownCommand>> = MaybeUninit::uninit();
static mut BG_TERMINAL: Option<WindowHandle> = None;

pub struct UserEnv;

impl UserEnv {
    pub fn start(f: fn()) {
        assert_call_once!();

        if !IS_GUI_BOOT {
            let point = 16;
            let font = FontDescriptor::new(FontFamily::Monospace, point)
                .unwrap_or(FontManager::monospace_font());

            let window = RawWindowBuilder::new()
                .style(WindowStyle::NO_SHADOW)
                .fullscreen()
                .level(WindowLevel::DESKTOP_ITEMS)
                .bg_color(Color::TRANSPARENT)
                .build("Terminal");

            unsafe {
                BG_TERMINAL = Some(window);
            }

            // WindowManager::set_desktop_color(Color::BLACK);
            let mut terminal = Terminal::from_window(
                window,
                Some(EdgeInsets::padding_each(4)),
                font,
                Alpha8::OPAQUE,
                0x07,
                Some(&[
                    IndexedColor::BLACK.into(),
                    IndexedColor::BLUE.into(),
                    IndexedColor::GREEN.into(),
                    IndexedColor::CYAN.into(),
                    IndexedColor::RED.into(),
                    IndexedColor::MAGENTA.into(),
                    IndexedColor::BROWN.into(),
                    IndexedColor::LIGHT_GRAY.into(),
                    IndexedColor::DARK_GRAY.into(),
                    IndexedColor::LIGHT_BLUE.into(),
                    IndexedColor::LIGHT_GREEN.into(),
                    IndexedColor::LIGHT_CYAN.into(),
                    IndexedColor::LIGHT_RED.into(),
                    IndexedColor::LIGHT_MAGENTA.into(),
                    IndexedColor::YELLOW.into(),
                    IndexedColor::WHITE.into(),
                ]),
            );
            terminal.reset().unwrap();
            System::set_stdout(Box::new(terminal));

            let device = System::current_device();

            let bytes = device.total_memory_size();
            let gb = bytes >> 30;
            let mb = (100 * (bytes & 0x3FFF_FFFF)) / 0x4000_0000;
            println!(
                "{} v{} (codename {}) {} Cores {}.{:02} GB Memory",
                System::name(),
                System::version(),
                System::codename(),
                device.num_of_main_cpus(),
                gb,
                mb
            );
        }

        unsafe {
            SHUTDOWN_COMMAND.write(EventQueue::new(100));
        }

        SpawnOption::with_priority(Priority::Normal)
            .start_process(Self::_main, f as usize, "init")
            .unwrap();

        let command = Self::shutdown_command().wait_event();

        WindowManager::set_pointer_enabled(false);
        WindowManager::set_barrier_opacity(Alpha8::TRANSPARENT);

        {
            let bounds = WindowManager::main_screen_bounds();
            let mut window_contents = OwnedBitmap32::new(bounds.size(), TrueColor::TRANSPARENT);
            WindowManager::save_screen_to(window_contents.as_mut(), bounds);
            let contents = window_contents
                .to_operational(|c| (c.brightness().unwrap_or_default() as usize) as u8);

            let bg_window = RawWindowBuilder::new()
                .style(WindowStyle::NO_SHADOW | WindowStyle::FULLSCREEN | WindowStyle::SUSPENDED)
                .level(WindowLevel::POPUP_BARRIER_BG)
                .bg_color(Color::BLUE)
                .build("");

            bg_window.draw(|bitmap| {
                contents.blt_to(bitmap, Point::new(0, 0), bitmap.bounds(), |level, _c| {
                    TrueColor::from_gray(level, Alpha8::OPAQUE).into()
                });
            });
            bg_window.show();
        }

        let width = 480;
        let height = 240;

        let window = RawWindowBuilder::new()
            .style(WindowStyle::NO_SHADOW)
            .size(Size::new(width, height))
            .bg_color(Color::TRANSPARENT)
            .level(WindowLevel::POPUP)
            .build("");

        window.draw(|bitmap| {
            bitmap.clear();
            let Some(font) = FontDescriptor::new(FontFamily::SansSerif, 36) else {
                return
            };
            AttributedString::new()
                .font(&font)
                .color(Color::WHITE)
                .middle_center()
                .shadow(Color::from_argb(0xFF333333), Movement::new(2, 2))
                .text("Shutting down")
                .draw_text(bitmap, bitmap.bounds(), 0);
        });

        let animation = AnimatedProp::new(0.0, 0.5, Duration::from_millis(500));

        window.create_timer(0, Duration::from_millis(1));
        window.show();

        while let Some(message) = window.wait_message() {
            match message {
                WindowMessage::Timer(timer_id) => match timer_id {
                    0 => {
                        WindowManager::set_barrier_opacity(animation.progress().into());

                        if animation.is_alive() {
                            window.create_timer(0, Duration::from_millis(50));
                        } else {
                            break;
                        }
                    }
                    _ => unreachable!(),
                },
                _ => window.handle_default_message(message),
            }
        }

        Timer::sleep(Duration::from_millis(200));

        let reboot = || unsafe {
            Hal::cpu().disable_interrupt();
            Scheduler::freeze(true);
            Hal::cpu().reset();
        };

        match command {
            ShutdownCommand::Reboot => reboot(),
            ShutdownCommand::Shutdown => {
                // TODO:
                reboot()
            }
        }

        unreachable!();
    }

    fn _main(f: usize) {
        let f: fn() = unsafe { transmute(f) };
        Scheduler::spawn_async(slpash_task(f));
        Scheduler::perform_tasks();
    }

    pub fn system_reset(shutdown: bool) {
        Self::shutdown_command()
            .post(if shutdown {
                ShutdownCommand::Shutdown
            } else {
                ShutdownCommand::Reboot
            })
            .unwrap();
    }

    fn shutdown_command<'a>() -> &'a EventQueue<ShutdownCommand> {
        unsafe { SHUTDOWN_COMMAND.assume_init_ref() }
    }
}

#[derive(Debug)]
enum ShutdownCommand {
    Reboot,
    Shutdown,
}

#[allow(dead_code)]
async fn slpash_task(f: fn()) {
    if IS_GUI_BOOT {
        if let Some(window) = unsafe { BG_TERMINAL.take() } {
            window.close();
        }

        let width = 480;
        let height = 240;

        let window = RawWindowBuilder::new()
            .style(WindowStyle::NO_SHADOW)
            .size(Size::new(width, height))
            .bg_color(Color::TRANSPARENT)
            .level(WindowLevel::POPUP)
            .build("");

        window.draw(|bitmap| {
            bitmap.clear();
            let Some(font) = FontDescriptor::new(FontFamily::SansSerif, 48) else {
                return
            };
            AttributedString::new()
                .font(&font)
                .color(Color::LIGHT_GRAY)
                .middle_center()
                .text("HELLO")
                .draw_text(bitmap, bitmap.bounds(), 0);
        });
        // window.show();
        WindowManager::set_barrier_opacity(Alpha8::OPAQUE);

        Timer::sleep_async(Duration::from_millis(1000)).await;

        Scheduler::spawn_async(status_bar_main());
        Scheduler::spawn_async(activity_monitor_main());
        Scheduler::spawn_async(_notification_task());

        if let Ok(mut file) = FileManager::open("/boot/wall.png", OpenOptions::new().read(true)) {
            let mut vec = Vec::new();
            file.read_to_end(&mut vec).unwrap();
            if let Ok(dib) = ImageLoader::load(vec.as_slice()) {
                WindowManager::set_desktop_bitmap(&dib.as_const());
            }
        } else {
            WindowManager::set_desktop_color(Theme::shared().default_desktop_color());
        }

        Timer::sleep_async(Duration::from_millis(500)).await;

        let animation = AnimatedProp::new(1.0, 0.0, Duration::from_millis(500));

        window.create_timer(0, Duration::from_millis(1));
        window.show();

        while let Some(message) = window.wait_message() {
            match message {
                WindowMessage::Timer(timer_id) => match timer_id {
                    0 => {
                        WindowManager::set_barrier_opacity(animation.progress().into());

                        if animation.is_alive() {
                            window.create_timer(0, Duration::from_millis(50));
                        } else {
                            window.close();
                        }
                    }
                    _ => unreachable!(),
                },
                _ => window.handle_default_message(message),
            }
        }

        WindowManager::set_barrier_opacity(Alpha8::TRANSPARENT);
    } else {
        Scheduler::spawn_async(_notification_task());
    }

    WindowManager::set_pointer_states(true, true, true);

    Scheduler::spawn_async(shell_launcher(f));

    // Scheduler::spawn_async(test_window_main());
}

#[allow(dead_code)]
async fn shell_launcher(f: fn()) {
    if IS_GUI_BOOT {
        Timer::sleep_async(Duration::from_millis(500)).await;

        // Main Terminal
        let size = WindowManager::main_screen_bounds();
        let point = if size.height() > 600 { 16 } else { 14 };
        let font = FontDescriptor::new(FontFamily::Monospace, point)
            .unwrap_or(FontManager::monospace_font());
        let terminal = Terminal::new(80, 24, font, None);
        System::set_stdout(Box::new(terminal));

        // Scheduler::spawn_async(clock_task());
    }
    SpawnOption::new().start_process(unsafe { core::mem::transmute(f) }, 0, "shell");
}

#[allow(dead_code)]
async fn status_bar_main() {
    const STATUS_BAR_HEIGHT: isize = 32;
    const STATUS_BAR_PADDING: EdgeInsets = EdgeInsets::new(0, 0, 0, 0);
    const INNER_PADDING: EdgeInsets = EdgeInsets::new(1, 24, 1, 24);

    let bg_color = Theme::shared().status_bar_background();
    let fg_color = Theme::shared().status_bar_foreground();

    let screen_bounds = WindowManager::main_screen_bounds();
    let window = RawWindowBuilder::new()
        .style(WindowStyle::NO_SHADOW | WindowStyle::FLOATING)
        .frame(Rect::new(0, 0, screen_bounds.width(), STATUS_BAR_HEIGHT))
        .bg_color(bg_color)
        .build("Status Bar");
    WindowManager::add_screen_insets(EdgeInsets::new(STATUS_BAR_HEIGHT, 0, 0, 0));

    let font = FontManager::monospace_font();
    let mut sb0 = Sb255::new();
    let mut sb1 = Sb255::new();

    window.create_timer(0, Duration::from_secs(0));
    while let Some(message) = window.await_message().await {
        match message {
            WindowMessage::Timer(_) => {
                let time = System::system_time();
                let tod = time.secs % 86400;
                let min = tod / 60 % 60;
                let hour = tod / 3600;
                sb0.clear();
                write!(sb0, "{:2}:{:02}", hour, min).unwrap();

                if sb0 != sb1 {
                    let ats = AttributedString::new()
                        .font(&font)
                        .color(fg_color)
                        .middle_center()
                        .text(sb0.as_str());

                    let bounds = Rect::from(window.content_size())
                        .insets_by(STATUS_BAR_PADDING)
                        .insets_by(INNER_PADDING);
                    let width = ats
                        .bounding_size(Size::new(isize::MAX, isize::MAX), 1)
                        .width;
                    let rect = Rect::new(
                        bounds.max_x() - width,
                        bounds.min_y(),
                        width,
                        bounds.height(),
                    );

                    window
                        .draw_in_rect(rect, |bitmap| {
                            bitmap.fill_rect(bitmap.bounds(), bg_color);
                            ats.draw_text(bitmap, bitmap.bounds(), 1);
                        })
                        .unwrap();

                    window.set_needs_display();
                    sb1 = sb0;
                }
                window.create_timer(0, Duration::from_millis(500));
            }
            _ => window.handle_default_message(message),
        }
    }
}

static mut ACTIVITY_WINDOW: Option<WindowHandle> = None;

fn format_bytes(sb: &mut dyn Write, val: usize) -> core::fmt::Result {
    let kb = (val >> 10) & 0x3FF;
    let mb = (val >> 20) & 0x3FF;
    let gb = val >> 30;

    if gb >= 10 {
        // > 10G
        write!(sb, "{:4}G", gb)
    } else if gb >= 1 {
        // 1G~10G
        let mb0 = (mb * 100) >> 10;
        write!(sb, "{}.{:02}G", gb, mb0)
    } else if mb >= 100 {
        // 100M~1G
        write!(sb, "{:4}M", mb)
    } else if mb >= 10 {
        // 10M~100M
        let kb00 = (kb * 10) >> 10;
        write!(sb, "{:2}.{}M", mb, kb00)
    } else if mb >= 1 {
        // 1M~10M
        let kb0 = (kb * 100) >> 10;
        write!(sb, "{}.{:02}M", mb, kb0)
    } else if kb >= 100 {
        // 100K~1M
        write!(sb, "{:4}K", kb)
    } else if kb >= 10 {
        // 10K~100K
        let b00 = ((val & 0x3FF) * 10) >> 10;
        write!(sb, "{:2}.{}K", kb, b00)
    } else {
        // 0~10K
        write!(sb, "{:5}", val)
    }
}

#[allow(dead_code)]
async fn activity_monitor_main() {
    let bg_color = Color::WHITE;
    let fg_color = Color::DARK_GRAY;
    let graph_border_color = Color::GREEN;
    let graph_sub_color = Color::LIGHT_GRAY;
    let graph_line_color = Color::LIGHT_MAGENTA;
    let graph_main_color1 = Color::LIGHT_RED;
    let graph_main_color2 = Color::LIGHT_GREEN;
    let graph_main_color3 = Color::LIGHT_GRAY;
    let margin = EdgeInsets::new(0, 0, 0, 0);

    let width = 260;
    let height = 180;
    let screen_bounds = WindowManager::user_screen_bounds();
    let window = RawWindowBuilder::new()
        .style_sub(WindowStyle::CLOSE_BUTTON)
        .frame(Rect::new(
            -width - 16,
            screen_bounds.min_y() + 16,
            width,
            height,
        ))
        .bg_color(bg_color)
        .build("Activity Monitor");

    unsafe {
        ACTIVITY_WINDOW = Some(window);
    }

    let font = FontDescriptor::new(FontFamily::SmallFixed, 8).unwrap_or(FontManager::ui_font());

    let num_of_cpus = System::current_device().num_of_logical_cpus();
    let n_items = 64;
    let mut usage_temp = Vec::with_capacity(num_of_cpus);
    let mut usage_cursor = 0;
    let mut usage_history = {
        let mut vec = Vec::with_capacity(n_items);
        vec.resize(n_items, u8::MAX);
        vec
    };

    let mut sb = String::new();

    let spacing = 4;
    let graph_rect = Rect::new(spacing, spacing, n_items as isize, 32);
    let graph_size = graph_rect.size();
    let meter_rect = Rect::new(
        graph_rect.max_x() + spacing,
        spacing,
        width - graph_rect.width() - 12,
        32,
    );
    let mut opr_bitmap = OperationalBitmap::new(graph_size);

    let interval = Duration::from_secs(1);
    window.create_timer(0, Duration::from_secs(0));
    while let Some(message) = window.await_message().await {
        match message {
            WindowMessage::Timer(_) => {
                Scheduler::get_idle_statistics(&mut usage_temp);
                let max_value = num_of_cpus as u32 * 1000;
                usage_history[usage_cursor] = (254
                    * u32::min(max_value, usage_temp.iter().fold(0, |acc, v| acc + *v))
                    / max_value) as u8;
                usage_cursor = (usage_cursor + 1) % n_items;

                window
                    .draw_in_rect(
                        Rect::from(window.content_size()).insets_by(margin),
                        |bitmap| {
                            bitmap.fill_rect(bitmap.bounds(), bg_color);

                            {
                                let h_lines = 4;
                                let v_lines = 4;
                                for i in 1..h_lines {
                                    let point = Point::new(
                                        graph_rect.min_x(),
                                        graph_rect.min_y() + i * graph_size.height / h_lines,
                                    );
                                    bitmap.draw_hline(point, graph_size.width, graph_sub_color);
                                }
                                for i in 1..v_lines {
                                    let point = Point::new(
                                        graph_rect.min_x() + i * graph_size.width / v_lines,
                                        graph_rect.min_y(),
                                    );
                                    bitmap.draw_vline(point, graph_size.height, graph_sub_color);
                                }

                                let limit = graph_rect.width() as usize - 2;
                                opr_bitmap.reset();
                                for i in 0..limit {
                                    let scale = graph_size.height - 2;
                                    let value1 = usage_history[(usage_cursor + i - limit) % n_items]
                                        as isize
                                        * scale
                                        / 255;
                                    let value2 = usage_history
                                        [(usage_cursor + i - 1 - limit) % n_items]
                                        as isize
                                        * scale
                                        / 255;
                                    let c1 = Point::new(i as isize + 1, 1 + value1);
                                    let c2 = Point::new(i as isize, 1 + value2);
                                    opr_bitmap.draw_line_anti_aliasing_f(
                                        c1,
                                        c2,
                                        |bitmap, point, level| unsafe {
                                            bitmap.process_pixel_unchecked(point, |c| {
                                                c.saturating_add((255.0 * level) as u8)
                                            });
                                        },
                                    );
                                    opr_bitmap.draw_line(
                                        Point::new(i as isize + 1, 1 + (value1 + value2) / 2),
                                        Point::new(i as isize + 1, graph_size.height - 1),
                                        |bitmap, point| unsafe {
                                            bitmap.process_pixel_unchecked(point, |c| {
                                                c.saturating_add(
                                                    0x20 + (graph_size.height - point.y) as u8 * 3,
                                                )
                                            });
                                        },
                                    );
                                }
                                opr_bitmap.draw_to(
                                    bitmap,
                                    graph_rect.origin(),
                                    graph_rect.bounds(),
                                    graph_line_color,
                                );
                                bitmap.draw_rect(graph_rect, graph_border_color);
                            }

                            // bitmap.draw_rect(meter_rect, graph_border_color);

                            for cpu_index in 0..num_of_cpus {
                                let rect = Rect::new(
                                    meter_rect.min_x() + cpu_index as isize * 8,
                                    meter_rect.min_y(),
                                    6,
                                    meter_rect.height(),
                                );

                                let value = usage_temp[cpu_index];
                                let graph_color = if value < 250 {
                                    graph_main_color1
                                } else if value < 750 {
                                    graph_main_color2
                                } else {
                                    graph_main_color3
                                };

                                let mut coords = Coordinates::from_rect(rect).unwrap();
                                coords.top += ((rect.height() - 1) * value as isize + 500) / 1000;

                                bitmap.fill_rect(coords.into(), graph_color);
                                bitmap.draw_rect(rect, graph_border_color);
                            }

                            sb.clear();

                            let device = System::current_device();

                            write!(sb, "Memory ").unwrap();
                            format_bytes(
                                &mut sb,
                                device.total_memory_size()
                                    - MemoryManager::free_memory_size()
                                    - MemoryManager::reserved_memory_size(),
                            )
                            .unwrap();
                            write!(sb, "B /").unwrap();
                            format_bytes(&mut sb, device.total_memory_size()).unwrap();
                            write!(sb, "B, ").unwrap();
                            format_bytes(&mut sb, MemoryManager::free_memory_size()).unwrap();
                            writeln!(sb, "B Free").unwrap();

                            let usage = Scheduler::usage_per_cpu();
                            let usage0 = usage % 10;
                            let usage1 = usage / 10;
                            write!(sb, "CPU: {:3}.{}%", usage1, usage0,).unwrap();

                            let n_cores = device.num_of_main_cpus();
                            let n_threads = device.num_of_logical_cpus();
                            if n_cores != n_threads {
                                write!(sb, " {}Cores {}Threads", n_cores, n_threads,).unwrap();
                            } else {
                                write!(sb, " {}Cores", n_cores,).unwrap();
                            }

                            writeln!(sb, " {:?}", Scheduler::current_state()).unwrap();

                            Scheduler::print_statistics(&mut sb);

                            let rect = bitmap
                                .bounds()
                                .insets_by(EdgeInsets::new(38, spacing, 4, spacing));
                            AttributedString::new()
                                .font(&font)
                                .color(fg_color)
                                .valign(VerticalAlignment::Top)
                                .text(sb.as_str())
                                .draw_text(bitmap, rect, 0);
                        },
                    )
                    .unwrap();

                window.set_needs_display();
                window.create_timer(0, interval);
            }
            WindowMessage::User(flag) => {
                let become_active = flag != 0;
                if become_active {
                    window.show();
                } else {
                    window.hide();
                }
            }
            _ => window.handle_default_message(message),
        }
    }
}

const NOTIFICATION_MESSAGE_ID: usize = 0;

/// Simple Notification Task
async fn _notification_task() {
    let window_width = 288;
    let window_height = 96;
    let margin = EdgeInsets::padding_each(8);
    let padding = EdgeInsets::padding_each(8);
    let item_spacing = 8;
    let radius = 12;

    let (bg_color, fg_color, border_color) = if false {
        (Color::from_argb(0xC0000000), Color::WHITE, Color::DARK_GRAY)
    } else {
        (
            Color::from_argb(0xF0cfd8dc),
            Color::BLACK,
            Theme::shared().window_default_border_light(),
        )
    };

    let window = RawWindowBuilder::new()
        .style(WindowStyle::FLOATING | WindowStyle::SUSPENDED)
        .level(WindowLevel::POPUP)
        .size(Size::new(window_width, window_height))
        .bg_color(Color::TRANSPARENT)
        .build("Notification Center");

    let message_buffer = Arc::new(ConcurrentFifo::with_capacity(100));
    Scheduler::spawn_async(_notification_observer(window, message_buffer.clone()));

    let dismiss_time = Duration::from_millis(5000);
    let mut last_timer = Timer::new(dismiss_time);

    const DISMISS_TIMER_ID: usize = 0;
    const OPEN_ANIMATION_TIMER_ID: usize = 1;
    const CLOSE_ANIMATION_TIMER_ID: usize = 2;

    let animation_duration = Duration::from_millis(150);
    let mut open_animation = AnimatedProp::empty();
    let mut close_animation = AnimatedProp::empty();

    let apply_frame = |window: WindowHandle, position: f64| {
        let main_screen_bounds = WindowManager::main_screen_bounds();
        let user_screen_bounds = WindowManager::user_screen_bounds();
        let rect = Rect::new(
            main_screen_bounds.width() - position as isize,
            user_screen_bounds.min_y(),
            window_width,
            window_height,
        )
        .insets_by(margin);
        window.set_frame(rect);
    };

    while let Some(message) = window.await_message().await {
        match message {
            WindowMessage::Timer(DISMISS_TIMER_ID) => {
                if last_timer.is_expired() {
                    close_animation =
                        AnimatedProp::new(window_width as f64, 0.0, animation_duration);
                    window.create_timer(CLOSE_ANIMATION_TIMER_ID, Duration::from_millis(1));
                }
            }
            WindowMessage::Timer(OPEN_ANIMATION_TIMER_ID) => {
                apply_frame(window, open_animation.progress());
                if open_animation.is_alive() {
                    window.create_timer(OPEN_ANIMATION_TIMER_ID, Duration::from_millis(10));
                }
            }
            WindowMessage::Timer(CLOSE_ANIMATION_TIMER_ID) => {
                apply_frame(window, close_animation.progress());
                if close_animation.is_alive() {
                    window.create_timer(CLOSE_ANIMATION_TIMER_ID, Duration::from_millis(10));
                } else {
                    window.hide();
                }
            }
            WindowMessage::User(NOTIFICATION_MESSAGE_ID) => {
                if let Some(payload) = message_buffer.dequeue() {
                    open_animation =
                        AnimatedProp::new(0.0, window_width as f64, animation_duration);
                    window.create_timer(OPEN_ANIMATION_TIMER_ID, Duration::from_millis(1));
                    apply_frame(window, open_animation.progress());

                    window
                        .draw_in_rect(Rect::from(window.content_size()), |bitmap| {
                            bitmap.clear();
                            let rect = bitmap.bounds();
                            bitmap.fill_round_rect(rect, radius, bg_color);
                            bitmap.draw_round_rect(rect, radius, border_color);

                            let mut left_margin = 0;
                            let rect = bitmap.bounds().insets_by(padding);

                            if let Some(ref icon) = IconManager::mask(payload.icon()) {
                                let long_side = usize::max(icon.width(), icon.height()) as isize;
                                let origin = Point::new(
                                    rect.min_x() + (long_side - icon.width() as isize) / 2,
                                    rect.min_y() + (rect.height() - long_side) / 2,
                                );
                                icon.draw_to(bitmap, origin, icon.bounds(), fg_color);

                                left_margin += item_spacing + long_side;
                            }

                            let rect2 = rect.insets_by(EdgeInsets::new(0, left_margin, 0, 0));
                            let ats = AttributedString::new()
                                .font(
                                    &FontDescriptor::new(FontFamily::SansSerif, 14)
                                        .unwrap_or(FontManager::ui_font()),
                                )
                                .color(fg_color)
                                .center()
                                .text(payload.message());
                            ats.draw_text(bitmap, rect2, 0);
                        })
                        .unwrap();

                    window.show();
                    last_timer = Timer::new(dismiss_time);
                    window.create_timer(DISMISS_TIMER_ID, dismiss_time);
                }
            }
            _ => window.handle_default_message(message),
        }
    }
}

async fn _notification_observer(
    window: WindowHandle,
    buffer: Arc<ConcurrentFifo<SimpleMessagePayload>>,
) {
    // Timer::sleep_async(Duration::from_millis(1000)).await;
    while let Some(payload) = EventManager::monitor_notification().await {
        buffer.enqueue(payload).unwrap();
        window
            .post(WindowMessage::User(NOTIFICATION_MESSAGE_ID))
            .unwrap();
        Timer::sleep_async(Duration::from_millis(3000)).await;
    }
}

#[allow(dead_code)]
async fn test_window_main() {
    let bg_color = Color::from_argb(0x80FFFFFF);
    // Timer::sleep_async(Duration::from_millis(500)).await;

    let width = 640;
    let height = 480;
    let window = RawWindowBuilder::new()
        .size(Size::new(width, height))
        .bg_color(bg_color)
        .inactive_title_color(bg_color)
        // .active_title_color(Color::LIGHT_BLUE)
        .level(WindowLevel::POPUP)
        .build("Welcome to ようこそ!");
    window.set_back_button_enabled(true);

    window.draw(|bitmap| {
        bitmap.fill_round_rect(bitmap.bounds(), 4, Color::WHITE);
        bitmap.draw_round_rect(bitmap.bounds(), 4, Color::LIGHT_GRAY);

        // let radius = 4;
        // bitmap.fill_round_rect(bitmap.bounds(), radius, Color::WHITE);
        // bitmap.draw_round_rect(bitmap.bounds(), radius, Color::LIGHT_GRAY);

        let font = FontManager::title_font();
        let title_height = 48;
        let button_width = 120;
        let button_height = 28;
        let button_radius = 8;
        let padding = 4;
        let padding_bottom = button_height;
        let button_center_top = Point::new(
            bitmap.bounds().mid_x(),
            bitmap.bounds().max_y() - padding_bottom - padding,
        );
        {
            let mut rect = bitmap.bounds();
            rect.size.height = title_height;
            bitmap
                .view(rect, |bitmap| {
                    let rect = bitmap.bounds();
                    bitmap.fill_rect(rect, Color::LIGHT_BLUE);
                    AttributedString::new()
                        .font(&FontDescriptor::new(FontFamily::SansSerif, 32).unwrap())
                        .middle_center()
                        .color(Color::WHITE)
                        .text("ようこそ MYOS!")
                        .draw_text(bitmap, rect, 1);
                })
                .unwrap();
        }
        {
            let rect = bitmap.bounds().insets_by(EdgeInsets::new(
                title_height + padding,
                4,
                padding_bottom + padding + padding,
                4,
            ));
            bitmap
                .view(rect, |bitmap| {
                    let mut offset = 0;
                    for family in [
                        FontFamily::SansSerif,
                        FontFamily::Serif,
                        FontFamily::Monospace,
                        // FontFamily::Cursive,
                    ] {
                        for point in [48, 32, 28, 24, 20, 16] {
                            offset += font_test(bitmap, offset, Color::BLACK, family, point);
                        }
                    }
                })
                .unwrap();
        }
        if true {
            let rect = Rect::new(
                button_center_top.x() - button_width - padding / 2,
                button_center_top.y(),
                button_width,
                button_height,
            );
            bitmap
                .view(rect, |bitmap| {
                    let rect = bitmap.bounds();
                    bitmap.fill_round_rect(
                        rect,
                        button_radius,
                        Theme::shared().button_default_background(),
                    );
                    // bitmap.draw_round_rect(
                    //     rect,
                    //     button_radius,
                    //     Theme::shared().button_default_border(),
                    // );
                    AttributedString::new()
                        .font(&font)
                        .middle_center()
                        .color(Theme::shared().button_default_foreground())
                        .text("Ok")
                        .draw_text(bitmap, rect, 1);
                })
                .unwrap();
        }
        if true {
            let rect = Rect::new(
                button_center_top.x() + padding / 2,
                button_center_top.y(),
                button_width,
                button_height,
            );
            bitmap
                .view(rect, |bitmap| {
                    let rect = bitmap.bounds();
                    bitmap.fill_round_rect(
                        rect,
                        button_radius,
                        Theme::shared().button_destructive_background(),
                    );
                    // bitmap.draw_round_rect(
                    //     rect,
                    //     button_radius,
                    //     Theme::shared().button_destructive_border(),
                    // );
                    AttributedString::new()
                        .font(&font)
                        .middle_center()
                        .color(Theme::shared().button_destructive_foreground())
                        .text("Cancel")
                        .draw_text(bitmap, rect, 1);
                })
                .unwrap();
        }
    });

    WindowManager::set_barrier_opacity(0.5.into());

    while let Some(message) = window.await_message().await {
        match message {
            WindowMessage::Close => {
                WindowManager::set_barrier_opacity(Alpha8::TRANSPARENT);
                window.close();
                return;
            }
            _ => window.handle_default_message(message),
        }
    }

    WindowManager::set_barrier_opacity(Alpha8::TRANSPARENT);
}

fn font_test(
    bitmap: &mut BitmapRefMut,
    offset: isize,
    color: Color,
    family: FontFamily,
    point: isize,
) -> isize {
    let max_lines = 0;
    let font = FontDescriptor::new(family, point).unwrap();
    let rect = Rect::new(0, offset, bitmap.width() as isize, isize::MAX);

    let ats = AttributedString::new()
        .font(&font)
        .top_left()
        .color(color)
        .line_break_mode(LineBreakMode::NoWrap)
        .shadow(TrueColor::from_argb(0x80CCCCCC).into(), Movement::new(2, 2))
        // .text("あのイーハトーヴォのすきとおった風、夏でも底に冷たさをもつ青いそら、うつくしい森で飾られたモリーオ市、郊外のぎらぎらひかる草の波。");
        // .text("The quick brown fox jumps over the lazy dog.");
        .text("WAVE AVATAR Lorem ipsum dolor sit amet, consectetur adipiscing elit,");

    let bounds = ats.bounding_size(rect.size(), max_lines);
    ats.draw_text(bitmap, rect, max_lines);

    bounds.height()
}

#[allow(dead_code)]
async fn clock_task() {
    let bg_color = Color::WHITE;
    let fg_color = Color::DARK_GRAY;

    let width = 240;
    let height = 240;
    let window_size = Size::new(width, height);
    let padding = 4;
    let radius = ((isize::min(width, height) - padding) / 2) as f64;
    let center = Point::new(width / 2, height / 2);
    let scale = 1.0;

    let window = RawWindowBuilder::new()
        .size(window_size)
        .bg_color(bg_color)
        .build("Retro Clock");

    let mut work_bitmap = OperationalBitmap::new(window_size);

    window.create_timer(0, Duration::from_millis(1));

    while let Some(message) = window.await_message().await {
        match message {
            WindowMessage::Timer(_id) => {
                window.set_needs_display();
                window.create_timer(0, Duration::from_millis(100));
            }
            WindowMessage::Draw => {
                let time = System::system_time();
                let seconds = time.secs as f64 + (time.nanos as f64 / 1_000_000_000.0);
                let h = time.secs as f64 / 3600.0 % 12.0;
                let m = time.secs as f64 / 60.0 % 60.0;
                let s = seconds % 60.0;

                work_bitmap.reset();

                for i in 0..60 {
                    let affine =
                        AffineMatrix2d::new(center.into(), Radian::TAU * (i as f64 / 60.0), scale);

                    if i % 15 == 0 {
                        let mut polygon = [
                            Vertex2d::new(0.0, 0.0 - radius),
                            Vertex2d::new(2.0, 4.0 - radius),
                            Vertex2d::new(0.0, 8.0 - radius),
                            Vertex2d::new(-2.0, 4.0 - radius),
                        ];
                        polygon.transform(&affine);
                        draw_polygon(&mut work_bitmap, &polygon, 0xFF);
                    } else if i % 5 == 0 {
                        let mut polygon = [
                            Vertex2d::new(0.0, 1.0 - radius),
                            Vertex2d::new(0.0, 7.0 - radius),
                        ];
                        polygon.transform(&affine);
                        draw_polygon(&mut work_bitmap, &polygon, 0xFF);
                    } else {
                        let mut polygon = [
                            Vertex2d::new(0.0, 2.0 - radius),
                            Vertex2d::new(0.0, 6.0 - radius),
                        ];
                        polygon.transform(&affine);
                        draw_polygon(&mut work_bitmap, &polygon, 0x55);
                    }
                }

                let mut polygon = [
                    Vertex2d::new(0.0, 0.0 - radius * 0.5),
                    Vertex2d::new(4.0, 0.0),
                    Vertex2d::new(0.0, 8.0),
                    Vertex2d::new(-4.0, 0.0),
                ];
                polygon.transform(&AffineMatrix2d::new(
                    center.into(),
                    Radian::TAU * h / 12.0,
                    scale,
                ));
                draw_polygon(&mut work_bitmap, &polygon, 0xCC);

                let mut polygon = [
                    Vertex2d::new(0.0, 16.0 - radius),
                    Vertex2d::new(2.0, 0.0),
                    Vertex2d::new(0.0, 4.0),
                    Vertex2d::new(-2.0, 0.0),
                ];
                polygon.transform(&AffineMatrix2d::new(
                    center.into(),
                    Radian::TAU * m / 60.0,
                    scale,
                ));
                draw_polygon(&mut work_bitmap, &polygon, 0xCC);

                let mut polygon = [Vertex2d::new(0.0, 16.0 - radius), Vertex2d::new(0.0, 4.0)];
                polygon.transform(&AffineMatrix2d::new(
                    center.into(),
                    Radian::TAU * s / 60.0,
                    scale,
                ));
                draw_polygon(&mut work_bitmap, &polygon, 0xEE);

                window.draw(|bitmap| {
                    bitmap.fill_rect(bitmap.bounds(), bg_color);

                    work_bitmap.draw_to(bitmap, Point::new(0, 0), work_bitmap.bounds(), fg_color);
                });
            }
            WindowMessage::Close => {
                window.close();
            }
            _ => window.handle_default_message(message),
        }
    }
}

fn draw_polygon(bitmap: &mut OperationalBitmap, polygon: &[Vertex2d], color: u8) {
    let mut polygon = polygon.iter();
    let len = polygon.len();
    let Some(vertex0) = polygon.next() else { return };
    let mut vertex1 = vertex0;
    while let Some(vertex2) = polygon.next() {
        bitmap.draw_line_anti_aliasing_f(
            (*vertex1).into(),
            (*vertex2).into(),
            |bitmap, point, level| unsafe {
                bitmap.process_pixel_unchecked(point, |c| {
                    c.saturating_add((color as f64 * level) as u8)
                });
            },
        );
        vertex1 = vertex2;
    }
    if len > 2 {
        bitmap.draw_line_anti_aliasing_f(
            (*vertex0).into(),
            (*vertex1).into(),
            |bitmap, point, level| unsafe {
                bitmap.process_pixel_unchecked(point, |c| {
                    c.saturating_add((color as f64 * level) as u8)
                });
            },
        );
    }
}
