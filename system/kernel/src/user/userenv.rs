use crate::{
    fs::*,
    io::tty::*,
    log::{EventManager, SimpleMessagePayload},
    mem::*,
    res::icon::IconManager,
    sync::fifo::ConcurrentFifo,
    system::*,
    task::scheduler::*,
    ui::font::*,
    ui::terminal::Terminal,
    ui::text::*,
    ui::theme::Theme,
    ui::window::*,
    *,
};
use ::alloc::{string::String, sync::Arc, vec::*};
use core::{fmt::Write, time::Duration};
use megstd::{drawing::image::ImageLoader, drawing::*, io::Read, string::*};

pub struct UserEnv;

impl UserEnv {
    pub fn start(f: fn()) {
        Scheduler::spawn_async(slpash_task(f));
        Scheduler::perform_tasks();
    }
}

async fn slpash_task(f: fn()) {
    let is_gui_boot = true;

    if true {
        let width = 320;
        let height = 200;

        let window = WindowBuilder::new()
            .style(WindowStyle::SUSPENDED)
            .bg_color(Color::Transparent)
            .size(Size::new(width, height))
            .build("Starting up...");

        window.draw(|bitmap| {
            let font = match FontDescriptor::new(FontFamily::SansSerif, 96) {
                Some(v) => v,
                None => return,
            };
            AttributedString::new()
                .font(font)
                .color(Color::LIGHT_GRAY)
                .middle_center()
                .text("Hello")
                .draw_text(bitmap, bitmap.bounds(), 0);
        });
        window.show();

        window.create_timer(0, Duration::from_millis(2000));

        while let Some(message) = window.await_message().await {
            match message {
                WindowMessage::Timer(_) => window.close(),
                _ => window.handle_default_message(message),
            }
        }
    }

    WindowManager::set_pointer_visible(true);

    if is_gui_boot {
        if let Ok(mut file) = FileManager::open("wall.qoi") {
            let mut vec = Vec::new();
            file.read_to_end(&mut vec).unwrap();
            if let Some(mut dib) = ImageLoader::from_qoi(vec.as_slice()) {
                WindowManager::set_desktop_bitmap(&dib.into_bitmap());
            }
        }
    }
    Timer::sleep_async(Duration::from_millis(500)).await;

    Scheduler::spawn_async(status_bar_main());
    Scheduler::spawn_async(_notification_task());
    Scheduler::spawn_async(activity_monitor_main());

    Scheduler::spawn_async(shell_launcher(is_gui_boot, f));

    // Scheduler::spawn_async(test_window_main());
}

#[allow(dead_code)]
async fn shell_launcher(is_gui_boot: bool, f: fn()) {
    if is_gui_boot {
        Timer::sleep_async(Duration::from_millis(500)).await;

        // Main Terminal
        let size = WindowManager::main_screen_bounds();
        let point = if size.height() > 600 { 16 } else { 14 };
        let font = FontDescriptor::new(FontFamily::Monospace, point)
            .unwrap_or(FontManager::monospace_font());
        let terminal = Terminal::new(80, 24, font, None);
        System::set_stdout(Box::new(terminal));
    } else {
        let size = WindowManager::main_screen_bounds();
        let max_point = isize::min(size.width() / 40, size.height() / 25);
        let min_point = max_point / 2;
        let point = {
            let mut point = max_point;
            while point > min_point {
                let font = match FontDescriptor::new(FontFamily::Monospace, point) {
                    Some(v) => v,
                    None => break,
                };
                if font.em_width() * 80 <= size.width() && font.line_height() * 25 <= size.height()
                {
                    break;
                }
                point -= 1;
            }
            point
        };
        let font = FontDescriptor::new(FontFamily::Monospace, point)
            .unwrap_or(FontManager::monospace_font());

        let window = WindowBuilder::new()
            .style(WindowStyle::NO_SHADOW)
            .fullscreen()
            .level(WindowLevel::DESKTOP_ITEMS)
            .bg_color(TrueColor::from_gray(0, 0).into())
            .build("Terminal");

        let mut terminal = Terminal::from_window(window, None, font, u8::MAX, 0, None);
        terminal.reset().unwrap();
        System::set_stdout(Box::new(terminal));
        // println!("Screen {} x {} Font {}", size.width(), size.height(), point);
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
    let window = WindowBuilder::new()
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
                        .font(font)
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
    let window = WindowBuilder::new()
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

    let num_of_cpus = System::current_device().num_of_active_cpus();
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
                                        graph_rect.x(),
                                        graph_rect.y() + i * graph_size.height / h_lines,
                                    );
                                    bitmap.draw_hline(point, graph_size.width, graph_sub_color);
                                }
                                for i in 1..v_lines {
                                    let point = Point::new(
                                        graph_rect.x() + i * graph_size.width / v_lines,
                                        graph_rect.y(),
                                    );
                                    bitmap.draw_vline(point, graph_size.height, graph_sub_color);
                                }

                                let limit = graph_rect.width() as usize - 2;
                                opr_bitmap.reset();
                                for i in 0..limit {
                                    let scale = graph_size.height - 2;
                                    let value1 = usage_history
                                        [((usage_cursor + i - limit) % n_items)]
                                        as isize
                                        * scale
                                        / 255;
                                    let value2 = usage_history
                                        [((usage_cursor + i - 1 - limit) % n_items)]
                                        as isize
                                        * scale
                                        / 255;
                                    let c0 = Point::new(i as isize + 1, 1 + value1);
                                    let c1 = Point::new(i as isize, 1 + value2);
                                    opr_bitmap.draw_line_anti_aliasing(
                                        c0,
                                        c1,
                                        1,
                                        |bitmap, point, level| unsafe {
                                            bitmap.set_pixel_unchecked(
                                                point,
                                                bitmap
                                                    .get_pixel_unchecked(point)
                                                    .saturating_add(level),
                                            );
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
                                    meter_rect.x() + cpu_index as isize * 8,
                                    meter_rect.y(),
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

                            let n_cores = device.num_of_performance_cpus();
                            let n_threads = device.num_of_active_cpus();
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
                                .font(font)
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

/// Simple Notification Task
async fn _notification_task() {
    let window_width = 288;
    let window_height = 96;
    let margin = EdgeInsets::padding_each(8);
    let padding = EdgeInsets::padding_each(8);
    let item_spacing = 8;
    let radius = 8;

    let (bg_color, fg_color, border_color) = if false {
        (Color::from_argb(0xC0000000), Color::WHITE, Color::DARK_GRAY)
    } else {
        (
            Theme::shared().window_title_active_background(),
            Theme::shared().window_title_active_foreground(),
            Theme::shared().window_default_border_light(),
        )
    };

    let screen_bounds = WindowManager::user_screen_bounds();
    let window = WindowBuilder::new()
        .style(WindowStyle::FLOATING | WindowStyle::SUSPENDED)
        .level(WindowLevel::POPUP)
        .frame(
            Rect::new(
                screen_bounds.max_x() - window_width,
                screen_bounds.min_y(),
                window_width,
                window_height,
            )
            .insets_by(margin),
        )
        .bg_color(Color::TRANSPARENT)
        .build("Notification Center");

    let message_buffer = Arc::new(ConcurrentFifo::with_capacity(100));
    Scheduler::spawn_async(_notification_observer(window, message_buffer.clone()));

    let dismiss_time = Duration::from_millis(5000);
    let mut last_timer = Timer::new(dismiss_time);

    while let Some(message) = window.await_message().await {
        match message {
            WindowMessage::Timer(_) => {
                if last_timer.is_expired() {
                    window.hide();
                }
            }
            WindowMessage::User(_) => {
                if let Some(payload) = message_buffer.dequeue() {
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
                                    FontDescriptor::new(FontFamily::SansSerif, 14)
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
                    window.create_timer(0, dismiss_time);
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
        window.post(WindowMessage::User(0)).unwrap();
        Timer::sleep_async(Duration::from_millis(3000)).await;
    }
}

#[allow(dead_code)]
async fn test_window_main() {
    let bg_color = Color::from_argb(0x80FFFFFF);
    // Timer::sleep_async(Duration::from_millis(500)).await;

    let width = 640;
    let height = 480;
    let window = WindowBuilder::new()
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
                        .font(FontDescriptor::new(FontFamily::SansSerif, 32).unwrap())
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
                        .font(font)
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
                        .font(font)
                        .middle_center()
                        .color(Theme::shared().button_destructive_foreground())
                        .text("Cancel")
                        .draw_text(bitmap, rect, 1);
                })
                .unwrap();
        }
    });

    WindowManager::set_barrier_opacity(0x80);

    while let Some(message) = window.await_message().await {
        match message {
            WindowMessage::Close => {
                WindowManager::set_barrier_opacity(0);
                window.close();
                return;
            }
            _ => window.handle_default_message(message),
        }
    }

    WindowManager::set_barrier_opacity(0);
}

fn font_test(
    bitmap: &mut Bitmap,
    offset: isize,
    color: Color,
    family: FontFamily,
    point: isize,
) -> isize {
    let max_lines = 0;
    let font = FontDescriptor::new(family, point).unwrap();
    let rect = Rect::new(0, offset, bitmap.width() as isize, isize::MAX);

    let ats = AttributedString::new()
        .font(font)
        .top_left()
        .color(color)
        .line_break_mode(LineBreakMode::NoWrap)
        // .text("あのイーハトーヴォのすきとおった風、夏でも底に冷たさをもつ青いそら、うつくしい森で飾られたモリーオ市、郊外のぎらぎらひかる草の波。");
        // .text("The quick brown fox jumps over the lazy dog.");
        .text("WAVE AVATAR Lorem ipsum dolor sit amet, consectetur adipiscing elit,");

    let bounds = ats.bounding_size(rect.size(), max_lines);
    ats.draw_text(bitmap, rect, max_lines);

    bounds.height()
}
