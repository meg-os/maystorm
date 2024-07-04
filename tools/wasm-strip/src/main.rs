// wasm-strip
// Copyright(c) 2021 The MEG-OS Project

use core::f64;
use std::env;
use std::fs::File;
use std::io::{Read, Write};
use std::path::Path;
use std::process;
use wasm_strip::wasm::*;

fn usage() -> ! {
    let mut args = env::args_os();
    let arg = args.next().unwrap();
    let path = Path::new(&arg);
    let lpc = path.file_name().unwrap();
    eprintln!("{} [OPTIONS] INPUT OUTPUT", lpc.to_str().unwrap());
    process::exit(1);
}

fn main() {
    let mut args = env::args();
    let _ = args.next().unwrap();

    let mut strip_all = false;
    let mut will_overwrite = false;
    let mut strip_names = Vec::new();
    let mut preserved_names = Vec::new();
    let mut path_input = None;
    let mut strip_export = false;

    while let Some(arg) = args.next() {
        if arg.starts_with("-") {
            match arg.as_str() {
                "-overwrite" => {
                    will_overwrite = true;
                }
                "-strip-all" => {
                    strip_all = true;
                }
                "-strip-export" => {
                    strip_export = true;
                }
                "-strip" => match args.next() {
                    Some(v) => strip_names.push(v),
                    None => usage(),
                },
                "-preserve" => match args.next() {
                    Some(v) => preserved_names.push(v),
                    None => usage(),
                },
                "--" => {
                    path_input = args.next();
                    break;
                }
                _ => panic!("unknown option: {}", arg),
            }
        } else {
            path_input = Some(arg);
            break;
        }
    }

    let path_input = match path_input {
        Some(v) => v,
        None => usage(),
    };
    let path_output = match args.next() {
        Some(v) => v,
        None => path_input.clone(),
    };

    // println!("FILE {} => {}", path_input, path_output);

    {
        let is_same_file = path_input == path_output;
        let mut ib = Vec::new();
        let mut is = File::open(path_input).expect("cannot open file");
        is.read_to_end(&mut ib).expect("read file error");
        drop(is);
        let org_size = ib.len();

        if !WasmMiniLoader::identify(ib.as_slice()) {
            panic!("bad signature found");
        }
        let sections = WasmMiniLoader::load_sections(ib.as_slice()).unwrap();

        let mut ob = Vec::with_capacity(org_size);
        ob.extend_from_slice(&WasmMiniLoader::file_header());

        for (index, section) in sections.iter().enumerate() {
            let preserved = match section.section_type() {
                WasmSectionType::Export => !strip_export,
                WasmSectionType::Custom => match section.custom_section_name() {
                    Some(name) => {
                        preserved_names.binary_search(&name).is_ok()
                            || !(strip_all
                                || strip_names.binary_search(&name).is_ok()
                                || name.starts_with("."))
                    }
                    None => false,
                },
                _ => true,
            };
            if preserved {
                section.write_to_vec(&mut ob);
            } else {
                println!(
                    "DROPPED section #{} {} ({:?} {}) file: {}, {}",
                    index,
                    section.section_type() as usize,
                    section.section_type(),
                    section.custom_section_name().unwrap_or("-".to_string()),
                    section.file_position(),
                    section.stream_size(),
                );
            }
        }

        let out_size = ob.len();

        if !will_overwrite && is_same_file && org_size <= out_size {
            println!("There is no more data in the file that can be stripped.");
        } else {
            let mut os = File::create(path_output).expect("cannot create file");
            os.write_all(&ob).expect("write error");
            drop(os);

            println!(
                " {} bytes <= {} bytes ({:.2}%)",
                out_size,
                org_size,
                (100.0 * out_size as f64) / org_size as f64
            );
        }
    }
}
