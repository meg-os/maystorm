// MEG-OS Kernel
// (c) 2020 Nerry
// License: MIT

#![no_std]
#![no_main]

extern crate alloc;
use alloc::str;
use bootprot::*;
use core::fmt::{self, Write};
use core::num::NonZeroU8;
use core::ptr::{addr_of, addr_of_mut};
use kernel::drivers::pci;
use kernel::drivers::usb;
use kernel::fs::*;
use kernel::init::SysInit;
use kernel::mem::*;
use kernel::rt::*;
use kernel::system::*;
use kernel::task::scheduler::*;
use kernel::ui::window::WindowManager;
use kernel::*;
use megstd::io::Read;
use megstd::path::Path;
use megstd::time::SystemTime;

/// Kernel entry point
#[no_mangle]
unsafe fn _start(info: &BootInfo) -> ! {
    system::System::init(info, Shell::start);
}

static mut MAIN: Shell = Shell::new();

const ENV_HOME: &str = "HOME";
const ENV_PATH: &str = "PATH";
const ENV_PATH_EXT: &str = "PATHEXT";
const SEP_PATH: &str = ":";

pub struct Shell {
    env: BTreeMap<String, String>,
}

enum ParsedCmdLine {
    Empty,
    InvalidQuote,
}

impl Shell {
    #[inline]
    const fn new() -> Self {
        Self {
            env: BTreeMap::new(),
        }
    }

    #[inline]
    unsafe fn shared_mut<'a>() -> &'a mut Self {
        unsafe { &mut *addr_of_mut!(MAIN) }
    }

    #[inline]
    fn shared<'a>() -> &'a Self {
        unsafe { &*addr_of!(MAIN) }
    }

    // Shell entry point
    fn start() {
        let shared = unsafe { Self::shared_mut() };
        let mut path_ext = Vec::new();
        for ext in RuntimeEnvironment::supported_extensions() {
            path_ext.push(format!(".{ext}"));
        }
        shared.set_env(ENV_PATH_EXT, &path_ext.join(SEP_PATH));

        shared.set_env(ENV_HOME, "/boot");
        shared.set_env(ENV_PATH, "/bin:/boot:/boot/bin:/boot/system/bin");

        Self::exec_cmd("cd");

        let _ = Self::exec_script("startup.txt");

        Scheduler::spawn_async(Self::repl_main());
        Scheduler::perform_tasks();
    }

    #[inline]
    fn set_env(&mut self, key: &str, value: &str) {
        self.env.insert(key.to_owned(), value.to_owned());
    }

    #[inline]
    fn get_env(&self, key: &str) -> Option<&str> {
        self.env.get(key).map(|v| v.as_str())
    }

    async fn repl_main() {
        let stdout = System::stdout();
        loop {
            let cwd = Scheduler::current_pid().cwd();
            let lpc = match Path::new(&cwd).file_name() {
                Some(v) => v,
                None => OsStr::new("/"),
            };
            let attributes = stdout.attributes();
            let text_bg = attributes & 0xF0;
            stdout.set_attribute(text_bg | 0x09);
            print!("{}", lpc.to_str().unwrap());
            stdout.set_attribute(0);
            print!("> ");
            if let Ok(cmdline) = System::stdout().read_line_async(120).await {
                Self::exec_cmd(&cmdline);
            }
        }
    }

    fn exec_script(path: &str) -> Result<(), megstd::io::Error> {
        let mut file = FileManager::open(path, OpenOptions::new().read(true))?;

        let mut buf = Vec::new();
        file.read_to_end(&mut buf)?;

        let script = core::str::from_utf8(&buf).map_err(|_| megstd::io::ErrorKind::InvalidData)?;

        for line in script.lines() {
            if !line.starts_with("#") {
                Self::exec_cmd(line);
            }
        }

        Ok(())
    }

    fn exec_cmd(cmdline: &str) {
        let mut cmdline = cmdline;
        let mut wait_until = true;
        if cmdline.ends_with("&") {
            wait_until = false;
            cmdline = &cmdline[..cmdline.len() - 1];
        }
        match Self::parse_cmd(cmdline) {
            Ok((cmd, args)) => {
                let name = cmd.as_str();
                let args = args.iter().map(|v| v.as_str()).collect::<Vec<&str>>();
                match name {
                    "clear" | "cls" | "reset" => {
                        System::stdout().reset().unwrap();
                    }
                    "exit" => {
                        println!("Feature not available");
                    }
                    "echo" => {
                        let stdout = System::stdout();
                        for (index, word) in args.iter().skip(1).enumerate() {
                            if index > 0 {
                                stdout.write_char(' ').unwrap();
                            }
                            stdout.write_str(word).unwrap();
                        }
                        stdout.write_str("\r\n").unwrap();
                    }
                    "ver" => {
                        println!(
                            "{} v{} ({})",
                            System::name(),
                            System::version(),
                            System::codename()
                        )
                    }
                    "reboot" => {
                        SysInit::system_reset(false);
                    }
                    "shutdown" => {
                        SysInit::system_reset(true);
                    }
                    "env" => {
                        let shared = unsafe { Self::shared_mut() };
                        if let Some(arg1) = args.get(1) {
                            if arg1.contains("=") {
                                let mut pair = arg1.split("=");
                                let key = pair.next().unwrap();
                                let value = pair.next().unwrap_or_default();
                                if value.is_empty() {
                                    shared.env.remove(key);
                                } else {
                                    shared.set_env(key, value);
                                }
                            } else {
                                let key = arg1;
                                let value = shared.get_env(key).unwrap_or_default();
                                println!("{key}={value}");
                            }
                        } else {
                            for key in shared.env.keys() {
                                let value = shared.get_env(key).unwrap_or_default();
                                println!("{key}={value}");
                            }
                        }
                    }
                    "printenv" => {
                        let shared = Self::shared();
                        for key in shared.env.keys() {
                            let value = shared.get_env(key).unwrap_or_default();
                            println!("{key}={value}");
                        }
                    }
                    "uptime" => {
                        let systime = System::system_time();
                        let systime = systime.duration_since(SystemTime::UNIX_EPOCH).unwrap();
                        let sec = systime.as_secs();
                        // let time_s = sec % 60;
                        let time_m = (sec / 60) % 60;
                        let time_h = (sec / 3600) % 24;

                        let uptime = Timer::monotonic();
                        let sec = uptime.as_secs();
                        let upt_s = sec % 60;
                        let upt_m = (sec / 60) % 60;
                        let upt_h = (sec / 3600) % 24;
                        let upt_d = sec / 86400;

                        if upt_d > 0 {
                            println!(
                                "{:02}:{:02} up {} days, {:02}:{:02}",
                                time_h, time_m, upt_d, upt_h, upt_m
                            );
                        } else {
                            println!(
                                "{:02}:{:02} up {:02}:{:02}:{:02}",
                                time_h, time_m, upt_h, upt_m, upt_s
                            );
                        }
                    }
                    "ts" => {
                        let mut sb = String::new();
                        Scheduler::get_thread_statistics(&mut sb);
                        print!("{}", sb.as_str());
                    }
                    "open" | "ncst" => {
                        let args = &args[1..];
                        let name = args[0];
                        Self::spawn(name, args, false);
                    }
                    _ => match Self::command(name) {
                        Some(exec) => {
                            exec(args.as_slice());
                        }
                        None => {
                            Self::spawn(name, args.as_slice(), wait_until);
                        }
                    },
                }
            }
            Err(ParsedCmdLine::Empty) => (),
            Err(ParsedCmdLine::InvalidQuote) => {
                println!("Error: Invalid quote");
            }
        }
    }

    fn parse_cmd(cmdline: &str) -> Result<(String, Vec<String>), ParsedCmdLine> {
        enum CmdLinePhase {
            SkippingSpace,
            Token,
            SingleQuote,
            DoubleQuote,
        }

        if cmdline.len() == 0 {
            return Err(ParsedCmdLine::Empty);
        }

        let mut sb = String::new();
        let mut args = Vec::new();
        let mut phase = CmdLinePhase::SkippingSpace;
        for c in cmdline.chars() {
            match phase {
                CmdLinePhase::SkippingSpace => match c {
                    ' ' | '\t' | '\r' | '\n' => (),
                    '\'' => {
                        phase = CmdLinePhase::SingleQuote;
                    }
                    '\"' => {
                        phase = CmdLinePhase::DoubleQuote;
                    }
                    _ => {
                        sb.write_char(c).unwrap();
                        phase = CmdLinePhase::Token;
                    }
                },
                CmdLinePhase::Token => match c {
                    ' ' | '\t' | '\r' | '\n' => {
                        args.push(sb);
                        phase = CmdLinePhase::SkippingSpace;
                        sb = String::new();
                    }
                    _ => {
                        sb.write_char(c).unwrap();
                    }
                },
                CmdLinePhase::SingleQuote => match c {
                    '\'' => {
                        args.push(sb);
                        phase = CmdLinePhase::SkippingSpace;
                        sb = String::new();
                    }
                    _ => {
                        sb.write_char(c).unwrap();
                    }
                },
                CmdLinePhase::DoubleQuote => match c {
                    '\"' => {
                        args.push(sb);
                        phase = CmdLinePhase::SkippingSpace;
                        sb = String::new();
                    }
                    _ => {
                        sb.write_char(c).unwrap();
                    }
                },
            }
        }
        match phase {
            CmdLinePhase::SkippingSpace | CmdLinePhase::Token => (),
            CmdLinePhase::SingleQuote | CmdLinePhase::DoubleQuote => {
                return Err(ParsedCmdLine::InvalidQuote)
            }
        }
        if sb.len() > 0 {
            args.push(sb);
        }
        if let Some(cmd) = args.get(0) {
            Ok((cmd.to_owned(), args))
        } else {
            Err(ParsedCmdLine::Empty)
        }
    }

    fn spawn(path: &str, argv: &[&str], wait_until: bool) -> usize {
        Self::spawn_main(path, argv, wait_until).unwrap_or_else(|| {
            let main_path = path;
            if !main_path.contains("/") {
                let mut sb = String::new();
                let shared = Self::shared();
                let path_ext = shared.get_env(ENV_PATH_EXT).unwrap_or_default();
                let path_ext = path_ext.split(SEP_PATH);
                let path = shared.get_env(ENV_PATH).unwrap_or_default();
                let path = path.split(SEP_PATH);
                for prefix in [""].into_iter().chain(path) {
                    for ext in path_ext.clone() {
                        sb.clear();
                        if prefix.is_empty() {
                            write!(sb, "{}{}", main_path, ext).unwrap();
                        } else {
                            write!(sb, "{}/{}{}", prefix, main_path, ext).unwrap();
                        }
                        match Self::spawn_main(sb.as_str(), argv, wait_until) {
                            Some(v) => return v,
                            None => (),
                        }
                    }
                }
            }
            println!("Command not found: {}", main_path);
            1
        })
    }

    fn spawn_main(path: &str, argv: &[&str], wait_until: bool) -> Option<usize> {
        match RuntimeEnvironment::spawn(path, argv) {
            Ok(child) => {
                if wait_until {
                    child.join();
                }
                Some(0)
            }
            Err(err) => match err.kind() {
                megstd::io::ErrorKind::NotFound => None,
                _ => {
                    println!("error {:?}", err);
                    Some(1)
                }
            },
        }
    }

    fn command(cmd: &str) -> Option<&'static fn(&[&str]) -> ()> {
        for command in &Self::COMMAND_TABLE {
            if command.0 == cmd {
                return Some(&command.1);
            }
        }
        None
    }

    const COMMAND_TABLE: [(&'static str, fn(&[&str]) -> (), &'static str); 17] = [
        ("cat", Self::cmd_cat, "Show a file"),
        ("cd", Self::cmd_cd, ""),
        ("dir", Self::cmd_ls, ""),
        ("help", Self::cmd_help, ""),
        ("ls", Self::cmd_ls, "Show list of directory"),
        ("lspci", Self::cmd_lspci, "Show list of PCI Devices"),
        ("lsusb", Self::cmd_lsusb, "Show list of USB Devices"),
        ("mkdir", Self::cmd_mkdir, ""),
        ("mount", Self::cmd_mount, ""),
        ("mv", Self::cmd_mv, ""),
        ("ps", Self::cmd_ps, ""),
        ("pwd", Self::cmd_pwd, ""),
        ("rm", Self::cmd_rm, ""),
        ("stat", Self::cmd_stat, ""),
        ("sysctl", Self::cmd_sysctl, "System Control"),
        ("touch", Self::cmd_touch, ""),
        ("type", Self::cmd_cat, ""),
    ];

    fn cmd_help(_: &[&str]) {
        for cmd in &Self::COMMAND_TABLE {
            if cmd.2.len() > 0 {
                println!("{}\t{}", cmd.0, cmd.2);
            }
        }
    }

    fn cmd_cd(argv: &[&str]) {
        let home = Self::shared().get_env(ENV_HOME).unwrap_or("/");
        let path = argv.get(1).unwrap_or(&home);
        match FileManager::chdir(path) {
            Ok(_) => (),
            Err(err) => {
                println!("cd: {}: {:?}", path, err.kind());
            }
        }
    }

    fn cmd_mkdir(argv: &[&str]) {
        let mut argv = argv.iter();
        let arg0 = unsafe { argv.next().unwrap_unchecked() };

        if argv.len() < 1 {
            println!("usage: {} directory_name", arg0);
            return;
        };

        for path in argv {
            match FileManager::mkdir(path) {
                Ok(_) => (),
                Err(err) => {
                    println!("{}: {}: {:?}", arg0, path, err.kind());
                }
            }
        }
    }

    fn cmd_rm(argv: &[&str]) {
        let mut argv = argv.iter();
        let arg0 = unsafe { argv.next().unwrap_unchecked() };

        if argv.len() < 1 {
            println!("usage: {} file", arg0);
            return;
        };

        for path in argv {
            match FileManager::unlink(path) {
                Ok(_) => (),
                Err(err) => {
                    println!("{}: {}: {:?}", arg0, path, err.kind());
                }
            }
        }
    }

    fn cmd_mv(argv: &[&str]) {
        let mut argv = argv.iter();
        let arg0 = unsafe { argv.next().unwrap_unchecked() };

        if argv.len() < 2 {
            println!("usage: {} source target", arg0);
            return;
        };

        let old_path = argv.next().unwrap();
        let new_path = argv.next().unwrap();
        match FileManager::rename(old_path, new_path) {
            Ok(_) => (),
            Err(err) => {
                println!("{}: {} to {}: {:?}", arg0, old_path, new_path, err.kind());
            }
        }
    }

    fn cmd_touch(argv: &[&str]) {
        let mut argv = argv.iter();
        let arg0 = unsafe { argv.next().unwrap_unchecked() };

        if argv.len() < 1 {
            println!("usage: {} file", arg0);
            return;
        };

        for path in argv {
            match FileManager::creat(path) {
                Ok(_) => (),
                Err(err) => {
                    println!("{}: {}: {:?}", arg0, path, err.kind());
                }
            }
        }
    }

    fn cmd_pwd(_argv: &[&str]) {
        println!("{}", Scheduler::current_pid().cwd());
    }

    fn cmd_sysctl(argv: &[&str]) {
        if argv.len() < 2 {
            println!("usage: sysctl command [options]");
            println!("memory:\tShow memory information");
            return;
        }

        fn print_cpu_type(device: &DeviceInfo, new_line: bool) {
            let n_threads = device.num_of_logical_cpus();
            let n_cores = device.num_of_physical_cpus();
            let n_pcores = device.num_of_main_cpus();
            let n_ecores = device.num_of_efficient_cpus();

            match device.processor_system_type() {
                ProcessorSystemType::Hybrid => {
                    print!(
                        "Hybrid {}P + {}E Core / {} Threads",
                        n_pcores, n_ecores, n_threads,
                    );
                }
                ProcessorSystemType::SMT => {
                    print!("SMT {} Cores / {} Threads", n_cores, n_threads,);
                }
                ProcessorSystemType::SMP => {
                    print!("SMP {} Processors", n_cores,);
                }
                ProcessorSystemType::Uniprocessor => {
                    print!("Uniprocessor");
                }
            }

            if new_line {
                println!("");
            }
        }

        let subcmd = argv[1];
        match subcmd {
            "device" => {
                let device = System::current_device();
                print_cpu_type(device, false);

                let bytes = device.total_memory_size();
                let gb = bytes >> 30;
                let mb = (100 * (bytes & 0x3FFF_FFFF)) / 0x4000_0000;
                println!(", Memory {}.{:02} GB", gb, mb);

                if let Some(manufacturer_name) = device.manufacturer_name() {
                    println!("  Manufacturer: {}", manufacturer_name);
                }
                if let Some(model_name) = device.model_name() {
                    println!("  Model: {}", model_name);
                }
            }
            "cpu" => {
                let device = System::current_device();
                print_cpu_type(device, true);

                for (index, cpu) in System::cpus().enumerate() {
                    println!(
                        "CPU #{} {:08x} {:?}",
                        index,
                        cpu.physical_id(),
                        cpu.processor_type()
                    );
                }
            }
            "memory" => {
                let mut sb = String::new();
                MemoryManager::statistics(&mut sb);
                print!("{}", sb.as_str());
            }
            "memmap" => {
                let mut sb = String::new();
                MemoryManager::get_memory_map(&mut sb);
                print!("{}", sb.as_str());
            }
            "windows" => {
                let mut sb = String::new();
                WindowManager::get_statistics(&mut sb);
                print!("{}", sb.as_str());
            }
            "drivers" => {
                for driver in pci::Pci::drivers() {
                    println!(
                        "PCI {:?} {} {}",
                        driver.address(),
                        driver.name(),
                        driver.current_status()
                    );
                }
            }
            _ => {
                println!("Unknown command: {}", subcmd);
                return;
            }
        }
    }

    fn cmd_ls(args: &[&str]) {
        let path = args.get(1).unwrap_or(&"");
        let dir = match FileManager::read_dir(path) {
            Ok(v) => v,
            Err(err) => {
                println!("{:?}", err.kind());
                return;
            }
        };

        let stdout = System::stdout();
        let attributes = stdout.attributes();
        let text_bg = attributes & 0xF0;
        // let text_fg = attributes & 0x0F;

        let mut files = dir
            .map(|v| {
                let metadata = v.metadata();
                let (color, suffix) = if metadata.file_type().is_dir() {
                    (text_bg | 0x09, "/")
                } else if metadata.file_type().is_symlink() {
                    (text_bg | 0x0D, "@")
                } else if metadata.file_type().is_char_device() {
                    (0x0E, "")
                } else {
                    (0, "")
                };
                (v.name().to_owned(), suffix, color)
            })
            .collect::<Vec<_>>();
        files.sort_by(|a, b| a.0.cmp(&b.0));

        let item_len = files.iter().fold(0, |acc, v| acc.max(v.0.len())) + 2;
        let width = stdout.dims().0 as usize;
        let items_per_line = width / item_len;
        let needs_new_line = items_per_line > 0 && width % item_len > 0;

        for (index, (name, suffix, attribute)) in files.into_iter().enumerate() {
            if (index % items_per_line) == 0 {
                if index > 0 && needs_new_line {
                    println!("");
                }
            }
            stdout.set_attribute(attribute);
            print!("{}", name);
            stdout.set_attribute(0);
            print!("{}", suffix);
            let len = name.len() + suffix.len();
            if len < item_len {
                print!("{:len$}", "", len = item_len - len);
            }
        }
        println!("");
    }

    fn cmd_cat(args: &[&str]) {
        let arg0 = args[0];
        let len = 0x10000;
        let mut sb = Vec::with_capacity(len);
        sb.resize(len, 0);
        for path in args.iter().skip(1) {
            let mut file = match FileManager::open(path, OpenOptions::new().read(true)) {
                Ok(v) => v,
                Err(err) => {
                    println!("{}: {}: {:?}", arg0, path, err.kind());
                    continue;
                }
            };
            loop {
                match file.read(sb.as_mut_slice()) {
                    Ok(0) => break,
                    Ok(size) => {
                        for b in &sb[..size] {
                            System::stdout().write_char(*b as char).unwrap();
                        }
                    }
                    Err(err) => {
                        println!("{}: {}: {:?}", arg0, path, err.kind());
                        break;
                    }
                }
            }
            // System::stdout().write_str("\r\n").unwrap();
        }
    }

    fn cmd_stat(args: &[&str]) {
        if args.len() < 2 {
            println!("stat PATH...");
            return;
        };
        for path in args.iter().skip(1) {
            let stat = match FileManager::stat(path) {
                Ok(v) => v,
                Err(err) => {
                    println!("stat: {}: {:?}", path, err.kind());
                    return;
                }
            };
            println!(
                "{} {:?} {} {}",
                stat.inode(),
                stat.file_type(),
                stat.len(),
                FileManager::canonicalize(path),
            )
        }
    }

    fn cmd_mount(_argv: &[&str]) {
        let mount_points = FileManager::mount_points();
        let mut keys = mount_points.keys().collect::<Vec<_>>();
        keys.sort();

        for key in keys {
            let mount_point = mount_points.get(key).unwrap();
            let description = mount_point.description().unwrap_or_default();
            println!("{} on {} {}", mount_point.device_name(), key, description);
        }
    }

    fn cmd_ps(_argv: &[&str]) {
        let mut sb = String::new();
        Scheduler::print_statistics(&mut sb);
        print!("{}", sb.as_str());
    }

    fn cmd_lsusb(argv: &[&str]) {
        if let Some(addr) = argv.get(1).and_then(|v| v.parse::<NonZeroU8>().ok()) {
            let addr = match usb::UsbAddress::from_nonzero(addr) {
                Some(v) => v,
                None => {
                    println!("Error: Bad usb address");
                    return;
                }
            };
            let device = match usb::UsbManager::device_by_addr(addr) {
                Some(v) => v,
                None => {
                    println!("Error: Device not found");
                    return;
                }
            };

            let class_string = device
                .class()
                .class_string(false)
                .unwrap_or("Unknown Device")
                .to_string();
            println!(
                "{:03} VID {} PID {} class {} USB {} {}",
                device.addr().as_u8(),
                device.vid(),
                device.pid(),
                device.class(),
                device.descriptor().usb_version(),
                class_string,
            );
            println!(
                "manufacturer: {}\nproduct: {}",
                device.manufacturer_string().unwrap_or("Unknown"),
                device.product_string().unwrap_or("Unknown"),
            );

            for config in device.configurations() {
                println!(
                    "config #{} {}",
                    config.configuration_value().0,
                    config.name().unwrap_or(""),
                );
                for interface in config.interfaces() {
                    let if_string = interface
                        .class()
                        .class_string(true)
                        .unwrap_or("Unknown Interface");
                    println!(
                        " interface #{}.{} class {:06x} {}",
                        interface.if_no().0,
                        interface.alternate_setting().0,
                        interface.class().0,
                        interface.name().unwrap_or(if_string),
                    );
                    for endpoint in interface.endpoints() {
                        println!(
                            "  endpoint {:02x} {:?} size {} interval {}",
                            endpoint.address().0,
                            endpoint.ep_type(),
                            endpoint.descriptor().max_packet_size().0,
                            endpoint.descriptor().interval(),
                        );
                    }
                }
            }
        } else {
            Self::print_usb_device(0, None);
        }
    }

    fn print_usb_device(level: usize, parent: Option<usb::UsbAddress>) {
        for device in usb::UsbManager::devices().filter(|v| v.parent() == parent) {
            println!(
                "{:indent$}{:03} VID {} PID {} class {} {}{}",
                "",
                device.addr().as_u8(),
                device.vid(),
                device.pid(),
                device.class(),
                if device.is_configured() { "" } else { "? " },
                device.preferred_device_name().unwrap_or("Unknown Device"),
                indent = level * 2
            );
            if device.children().next().is_some() {
                Self::print_usb_device(level + 1, Some(device.addr()));
            }
        }
    }

    fn cmd_lspci(_argv: &[&str]) {
        // let _opt_all = argv.len() > 1;
        for device in drivers::pci::Pci::devices() {
            let addr = device.address();
            let class_string = Self::find_pci_class_string(device.class_code());
            println!(
                "{:02x}:{:02x}.{} {:04x}:{:04x} {:06x} {}",
                addr.get_bus(),
                addr.get_dev(),
                addr.get_fun(),
                device.vendor_id().0,
                device.device_id().0,
                device.class_code().data(),
                class_string,
            );
        }
    }

    fn find_pci_class_string(cc: pci::PciClass) -> &'static str {
        use pci::PciClass;
        #[rustfmt::skip]
        let entries = [
            (PciClass::code(0x00).sub(0x00), "Non-VGA-Compatible devices"),
            (PciClass::code(0x00).sub(0x01), "VGA-Compatible Device"),
            (PciClass::code(0x01).sub(0x00), "SCSI Bus Controller"),
            (PciClass::code(0x01).sub(0x01), "IDE Controller"),
            (PciClass::code(0x01).sub(0x04), "Raid Controller"),
            (PciClass::code(0x01).sub(0x05), "ATA Controller"),
            (PciClass::code(0x01).sub(0x06).interface(0x01), "AHCI 1.0"),
            (PciClass::code(0x01).sub(0x06), "Serial ATA"),
            (PciClass::code(0x01).sub(0x07).interface(0x00), "SAS"),
            (PciClass::code(0x01).sub(0x07), "Serial Attached SCSI"),
            (PciClass::code(0x01).sub(0x08).interface(0x01), "NVMHCI"),
            (PciClass::code(0x01).sub(0x08).interface(0x02), "NVM Express"),
            (PciClass::code(0x01).sub(0x08), "Non-Volatile Memory Controller"),
            (PciClass::code(0x01), "Mass Storage Controller"),
            (PciClass::code(0x02).sub(0x00), "Ethernet Controller"),
            (PciClass::code(0x02), "Network Controller"),
            (PciClass::code(0x03).sub(0x00), "VGA Compatible Controller"),
            (PciClass::code(0x03), "Display Controller"),
            (PciClass::code(0x04).sub(0x00), "Multimedia Video Controller"),
            (PciClass::code(0x04).sub(0x01), "Multimedia Audio Controller"),
            (PciClass::code(0x04).sub(0x02), "Computer Telephony Device"),
            (PciClass::code(0x04).sub(0x03), "HD Audio Controller"),
            (PciClass::code(0x04), "Multimedia Controller"),
            (PciClass::code(0x05).sub(0x00), "RAM Controller"),
            (PciClass::code(0x05).sub(0x01), "Flash Controller"),
            (PciClass::code(0x05), "Memory Controller"),
            (PciClass::code(0x06).sub(0x00), "Host Bridge"),
            (PciClass::code(0x06).sub(0x01), "ISA Bridge"),
            (PciClass::code(0x06).sub(0x02), "EISA Bridge"),
            (PciClass::code(0x06).sub(0x03), "MCA Bridge"),
            (PciClass::code(0x06).sub(0x04).interface(0x00), "PCI-to-PCI Bridge (Normal Decode)"),
            (PciClass::code(0x06).sub(0x04).interface(0x01), "PCI-to-PCI Bridge (Subtractive Decode)"),
            (PciClass::code(0x06).sub(0x05), "PCMCIA Bridge"),
            (PciClass::code(0x06).sub(0x06), "NuBus Bridge"),
            (PciClass::code(0x06).sub(0x07), "CardBus Bridge"),
            (PciClass::code(0x06).sub(0x08), "RACEway Bridge"),
            (PciClass::code(0x06).sub(0x09), "PCI-to-PCI Bridge"),
            (PciClass::code(0x06).sub(0x0A), "InfiniBand-to-PCI Host Bridge"),
            (PciClass::code(0x06), "Bridge Device"),
            (PciClass::code(0x07).sub(0x00), "Serial Controller"),
            (PciClass::code(0x07).sub(0x01), "Parallel Controller"),
            (PciClass::code(0x07).sub(0x03), "Modem"),
            (PciClass::code(0x07).sub(0x04), "IEEE 488.1/2 (GPIB) Controller"),
            (PciClass::code(0x07).sub(0x05), "Smart Card"),
            (PciClass::code(0x07), "Simple Communication Controller"),
            (PciClass::code(0x08).sub(0x05), "SD Host controller"),
            (PciClass::code(0x08).sub(0x06), "IOMMU"),
            (PciClass::code(0x08), "Base System Peripheral"),
            (PciClass::code(0x09), "Input Device Controller"),
            (PciClass::code(0x0A), "Docking Station"),
            (PciClass::code(0x0B), "Processor"),
            (PciClass::code(0x0C).sub(0x03).interface(0x00), "UHCI Controller"),
            (PciClass::code(0x0C).sub(0x03).interface(0x10), "OHCI Controller"),
            (PciClass::code(0x0C).sub(0x03).interface(0x20), "EHCI Controller"),
            (PciClass::code(0x0C).sub(0x03).interface(0x30), "XHCI Controller"),
            (PciClass::code(0x0C).sub(0x03).interface(0x40), "USB4 Host Controller"),
            (PciClass::code(0x0C).sub(0x03), "USB Controller"),
            (PciClass::code(0x0C).sub(0x04), "Fibre Channel"),
            (PciClass::code(0x0C).sub(0x05), "SMBus"),
            (PciClass::code(0x0C).sub(0x06), "InfiniBand"),
            (PciClass::code(0x0C).sub(0x03), "USB Controller"),
            (PciClass::code(0x0C), "Serial Bus Controller"),
            (PciClass::code(0x0D).sub(0x11), "Bluetooth Controller"),
            (PciClass::code(0x0D), "Wireless Controller"),
            (PciClass::code(0x0E), "Intelligent Controller"),
            (PciClass::code(0x0F), "Satellite Communication Controller"),
            (PciClass::code(0x10), "Encryption Controller"),
            (PciClass::code(0x11), "Signal Processing Controller"),
            (PciClass::code(0x12), "Processing Accelerator"),
            (PciClass::code(0x13), "Encryption Controller"),
            (PciClass::code(0x14), "Non-Essential Instrumentation"),
            (PciClass::code(0x40), "Co-Processor"),
            (PciClass::code(0xFF), "(Vendor specific)"),
        ];
        for entry in &entries {
            if cc.matches(entry.0) {
                return entry.1;
            }
        }
        "(Unknown Device)"
    }

    #[allow(dead_code)]
    fn format_si(sb: &mut dyn fmt::Write, val: usize) -> core::fmt::Result {
        let kb = (val / 1000) % 1000;
        let mb = (val / 1000_000) % 1000;
        let gb = val / 1000_000_000;

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
    fn format_bytes(sb: &mut dyn fmt::Write, val: usize) -> core::fmt::Result {
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
}
