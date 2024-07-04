// Make an initrd image
// Copyright(c) 2021 The MEG-OS Project

use myos_archive::*;
use std::{
    cmp, env,
    ffi::{OsStr, OsString},
    fs::{read_dir, File},
    io::Read,
    io::Write,
    path::Path,
    process,
};

fn usage() -> ! {
    let mut args = env::args_os();
    let arg = args.next().unwrap();
    let path = Path::new(&arg);
    let lpc = path.file_name().unwrap();
    eprintln!("{} [OPTIONS] OUTPUT [FILES...]", lpc.to_str().unwrap());
    process::exit(1);
}

fn main() {
    let mut args = env::args();
    let _ = args.next().unwrap();

    let mut path_output = None;
    let mut is_verbose = false;

    while let Some(arg) = args.next() {
        let arg = arg.as_str();
        if arg.starts_with("-") {
            match arg {
                "-v" => is_verbose = true,
                "--" => {
                    path_output = args.next();
                    break;
                }
                _ => panic!("unknown option: {}", arg),
            }
        } else {
            path_output = Some(arg.to_owned());
            break;
        }
    }

    let path_output = match path_output {
        Some(v) => v,
        None => usage(),
    };

    println!("CREATING archive: {}", path_output);

    let mut files = Vec::new();
    for arg in args {
        append_path(&mut files, "", OsStr::new(&arg));
    }
    files.sort_by(|a, b| {
        let lhs = Path::new(&a.0);
        let rhs = Path::new(&b.0);
        match lhs
            .parent()
            .unwrap_or(Path::new(""))
            .cmp(rhs.parent().unwrap_or(Path::new("")))
        {
            cmp::Ordering::Equal => lhs.cmp(&rhs),
            result => result,
        }
    });

    let mut writer = ArchiveWriter::new();
    let mut cwd = "".to_owned();
    let mut n_ns = 0;
    for (path, os_path) in &files {
        let path = Path::new(&path);
        let lpc = path.file_name().unwrap().to_str().unwrap();
        let dir = path
            .parent()
            .and_then(|v| v.to_str())
            .map(|v| &v[1..])
            .unwrap_or("");
        if cwd != dir {
            if is_verbose {
                let old = cwd;
                println!("NAMESPACE: [{dir}] <= [{old}]");
            }
            writer
                .write(Entry::Namespace(&dir, ExtendedAttributes::empty()))
                .unwrap();
            cwd = dir.to_owned();
            n_ns += 1;
        }

        if is_verbose {
            println!(
                "FILE: {} ({})",
                &path.to_str().unwrap()[1..],
                os_path.to_str().unwrap()
            );
        }

        let mut buf = Vec::new();
        let mut is = File::open(os_path).expect("cannot open file");
        is.read_to_end(&mut buf).expect("read file error");

        writer
            .write(Entry::File(lpc, ExtendedAttributes::empty(), &buf))
            .unwrap();
    }

    let vec = writer.finalize(&[]).unwrap();
    let mut os = File::create(path_output).unwrap();
    os.write_all(&vec).unwrap();

    println!(
        " - TOTAL: {} files, {} bytes, {} namespaces",
        files.len(),
        vec.len(),
        n_ns
    );
}

#[allow(dead_code)]
fn append_path(vec: &mut Vec<(String, OsString)>, prefix: &str, path: &OsStr) {
    let path = Path::new(path);
    let lpc = path.file_name().unwrap().to_str().unwrap();
    if path.is_dir() {
        for entry in read_dir(path).unwrap() {
            let prefix = format!("{prefix}/{lpc}");
            let entry = entry.unwrap();
            let path = entry.path();
            append_path(vec, &prefix, path.as_os_str());
        }
    } else if path.is_file() {
        match lpc {
            // Bad files
            ".DS_Store" | "Thumbs.db" => (),
            _ => {
                let mut needs_to_push = true;
                let vpath = format!("{prefix}/{lpc}");
                for item in vec.iter_mut() {
                    if item.0 == vpath {
                        item.1 = path.as_os_str().to_owned();
                        needs_to_push = false;
                        break;
                    }
                }
                if needs_to_push {
                    vec.push((vpath, path.as_os_str().to_owned()))
                }
            }
        }
    } else {
        if path.ends_with("*") {
            //
        } else {
            todo!()
        }
    }
}
