// Make a floppy disk image
// Copyright(c) 2021 The MEG-OS Project

pub mod fat;

use fat::*;
use std::{
    env,
    fs::File,
    io::{Read, Write},
    mem::size_of,
    num::NonZeroU32,
    path::Path,
    process,
    ptr::addr_of,
    usize,
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

    let mut volume_label = None;
    let mut current_bpb = None;
    let mut path_bootsector = None;
    let mut path_output = None;

    while let Some(arg) = args.next() {
        let arg = arg.as_str();
        if arg.starts_with("-") {
            match arg {
                "--" => {
                    path_output = args.next();
                    break;
                }
                "-bs" => {
                    path_bootsector = Some(args.next().expect("needs boot sector file name"));
                }
                "-f" => {
                    let opt = args.next().expect("needs format type");
                    current_bpb = Some(parse_type(opt.as_str()).expect("unknown format type"));
                }
                "-touch" => {
                    // TODO:
                }
                "-l" => {
                    volume_label = Some(args.next().expect("needs volume label"));
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

    let mut boot_sector = if let Some(path_bootsector) = path_bootsector {
        let mut boot_sector = [0; BootSector::PREFERRED_SIZE];
        let mut is = File::open(path_bootsector).unwrap();
        is.read_exact(&mut boot_sector).unwrap();
        BootSector::from_bytes(boot_sector)
    } else {
        BootSector::default()
    };

    if let Some(bpb) = current_bpb {
        boot_sector.ebpb.bpb = bpb;
    }

    let mut root_dir = Vec::new();

    if let Some(volume_label) = volume_label {
        let dir_ent =
            DosDirEnt::volume_label(volume_label.as_str()).expect("invalid char in volume label");
        if boot_sector.ebpb.is_valid() {
            boot_sector.ebpb.volume_label = dir_ent.name;
        }
        root_dir.push(dir_ent);
    }

    let mut fs = Fatfs::from_bpb(&boot_sector.ebpb);
    let mut vd = VirtualDisk::new(&boot_sector, fs.sector_size, fs.total_sectors);
    fs.append_root_dir(root_dir.as_slice());

    let n_heads = unsafe { addr_of!(boot_sector.ebpb.bpb.n_heads).read_unaligned() as usize };
    let sectors_per_track =
        unsafe { addr_of!(boot_sector.ebpb.bpb.sectors_per_track).read_unaligned() as usize };
    println!(
        "CREATING image: {} KB [CHR {} {} {}] {} b/sec {} b/rec total {}",
        (fs.total_sectors * fs.sector_size) / 1024,
        fs.total_sectors / (n_heads * sectors_per_track),
        n_heads,
        sectors_per_track,
        fs.sector_size,
        fs.record_size,
        fs.total_records
    );

    for arg in args {
        let path = Path::new(&arg);
        let lpc = path.file_name().unwrap();
        let basename = lpc.to_str().unwrap();
        println!("COPYING: {} <= {}", basename, arg);

        let mut dir_ent = DosDirEnt::file_entry(basename).expect("file name");

        let mut buf = Vec::new();
        {
            let mut is = File::open(path).expect("cannot open file");
            is.read_to_end(&mut buf).expect("read file error");
        }
        let file_size = buf.len() as u32;
        dir_ent.file_size = file_size;
        if let Some(file_size) = NonZeroU32::new(file_size) {
            let first_record =
                fs.allocate(file_size).expect("file allocation error").get() as FatEntry;
            dir_ent.first_cluster = first_record;
            fs.write_file(&mut vd, first_record, buf.as_slice())
                .expect("file i/o error");
        }

        fs.append_root_dir(&[dir_ent]);
    }

    fs.flush(&mut vd).unwrap();
    let mut os = File::create(path_output).unwrap();
    vd.flush(&mut os).unwrap();
}

pub fn parse_type(opt: &str) -> Option<DosBpb> {
    match opt {
        "2hd" | "1440" => Some(DosBpb::new(512, 1, 1, 2, 224, 80 * 2 * 18, 0xF0, 9, 18, 2)),
        "2hc" | "1200" => Some(DosBpb::new(512, 1, 1, 2, 224, 80 * 2 * 15, 0xF9, 7, 15, 2)),
        "nec" | "1232" => Some(DosBpb::new(1024, 1, 1, 2, 192, 77 * 2 * 8, 0xFE, 2, 8, 2)),
        "2dd" | "720" => Some(DosBpb::new(512, 2, 1, 2, 112, 80 * 2 * 9, 0xF9, 3, 9, 2)),
        "640" => Some(DosBpb::new(512, 2, 1, 2, 112, 80 * 2 * 8, 0xFB, 2, 8, 2)),
        "320" => Some(DosBpb::new(512, 2, 1, 2, 112, 40 * 2 * 8, 0xFF, 2, 8, 2)),
        "160" => Some(DosBpb::new(512, 1, 1, 2, 64, 40 * 1 * 8, 0xFE, 1, 8, 1)),
        _ => None,
    }
}

type FatEntry = u16;

struct Fatfs {
    sector_size: usize,
    total_sectors: usize,
    record_size: usize,
    total_records: usize,
    offset_fat: usize,
    offset_root: usize,
    offset_cluster: usize,
    last_record_allocated: usize,
    fattype: FatType,
    end_of_chain: FatEntry,
    bpb: DosBpb,
    fat: Vec<FatEntry>,
    root_dir: Vec<DosDirEnt>,
}

#[allow(dead_code)]
enum FatType {
    Fat12,
    Fat16,
    Fat32,
}

impl Fatfs {
    fn from_bpb(ebpb: &DosExtendedBpb) -> Self {
        let bpb = ebpb.bpb;
        let sector_size = bpb.bytes_per_sector as usize;
        let total_sectors = if ebpb.is_valid() && ebpb.total_sectors32 > bpb.total_sectors as u32 {
            ebpb.total_sectors32 as usize
        } else {
            bpb.total_sectors as usize
        };

        let record_size = sector_size as usize * bpb.sectors_per_cluster as usize;
        let offset_fat = bpb.reserved_sectors_count as usize;
        let offset_root = offset_fat + (bpb.n_fats as usize * bpb.sectors_per_fat as usize);
        let offset_cluster =
            offset_root + (bpb.root_entries_count as usize * 32 + sector_size - 1) / sector_size;
        let total_records = (total_sectors - offset_cluster) / bpb.sectors_per_cluster as usize;

        let fattype;
        if total_records < 4085 {
            fattype = FatType::Fat12;
        } else if total_records < 65525 {
            fattype = FatType::Fat16;
        } else {
            // fattype = FatType::Fat32;
            unimplemented!();
        }

        let mut fat = Vec::with_capacity(2 + total_records);
        let end_of_chain = FatEntry::MAX;
        fat.resize(2 + total_records, 0);
        fat[0] = (end_of_chain & !0xFF) | bpb.media_descriptor as u16;
        fat[1] = end_of_chain;

        Self {
            sector_size,
            total_sectors,
            record_size,
            total_records,
            offset_fat,
            offset_root,
            offset_cluster,
            last_record_allocated: 2,
            fattype,
            end_of_chain,
            bpb: bpb.clone(),
            fat,
            root_dir: Vec::with_capacity(bpb.root_entries_count as usize),
        }
    }

    fn flush(&self, vd: &mut VirtualDisk) -> Result<(), VirtualDiskError> {
        let sectors_per_fat = self.bpb.sectors_per_fat as usize;
        match self.fattype {
            FatType::Fat12 => {
                let fat_size = (self.fat.len() * 3 + 1) / 2;
                let mut fat: Vec<u8> = Vec::with_capacity(fat_size);
                fat.resize(fat_size, 0);
                for (i, entry) in self.fat.iter().enumerate() {
                    let index = i * 3 / 2;
                    if (i & 1) == 0 {
                        fat[index] = *entry as u8;
                        fat[index + 1] = 0x0F & (*entry >> 8) as u8;
                    } else {
                        fat[index] |= (*entry << 4) as u8;
                        fat[index + 1] = (*entry >> 4) as u8;
                    }
                }
                vd.write(self.offset_fat, fat.as_slice())?;
                vd.write(self.offset_fat + sectors_per_fat, fat.as_slice())?;
            }
            FatType::Fat16 => {
                vd.write(self.offset_fat, self.fat.as_slice())?;
                vd.write(self.offset_fat + sectors_per_fat, self.fat.as_slice())?;
            }
            _ => unimplemented!(),
        }

        vd.write(self.offset_root, self.root_dir.as_slice())?;

        Ok(())
    }

    fn append_root_dir(&mut self, entries: &[DosDirEnt]) {
        self.root_dir.extend(entries.iter());
    }

    fn allocate(&mut self, file_size: NonZeroU32) -> Option<NonZeroU32> {
        let record_count = (file_size.get() as usize + self.record_size - 1) / self.record_size;
        if self.last_record_allocated + record_count < self.total_records {
            let first_record = self.last_record_allocated;
            self.last_record_allocated = self.last_record_allocated + record_count;
            if record_count > 1 {
                for i in 0..record_count - 1 {
                    let index = first_record + i;
                    self.fat[index] = index as FatEntry + 1;
                }
                self.fat[first_record + record_count - 1] = self.end_of_chain;
            } else {
                self.fat[first_record] = self.end_of_chain;
            }
            NonZeroU32::new(first_record as u32)
        } else {
            None
        }
    }

    fn record_to_sector(&self, record: FatEntry) -> usize {
        self.offset_cluster + (record as usize - 2) * self.bpb.sectors_per_cluster as usize
    }

    fn write_file(
        &self,
        vd: &mut VirtualDisk,
        offset: FatEntry,
        data: &[u8],
    ) -> Result<usize, VirtualDiskError> {
        let lba = self.record_to_sector(offset);
        vd.write(lba, data)
    }
}

pub struct VirtualDisk {
    vec: Vec<u8>,
    sector_size: usize,
    total_sector: usize,
}

impl VirtualDisk {
    pub fn new(boot_sector: &BootSector, sector_size: usize, total_sector: usize) -> Self {
        let capacity = sector_size * total_sector;
        let mut vec = Vec::with_capacity(capacity);
        vec.extend_from_slice(boot_sector.as_bytes());
        vec.resize(capacity, 0);
        Self {
            vec,
            sector_size,
            total_sector,
        }
    }

    pub fn flush(&self, os: &mut dyn Write) -> Result<(), VirtualDiskError> {
        os.write_all(self.vec.as_slice())
            .map(|_| ())
            .map_err(|_| VirtualDiskError::IoError)
    }

    pub fn write<T>(&mut self, lba: usize, data: &[T]) -> Result<usize, VirtualDiskError>
    where
        T: Sized,
    {
        let data_size = data.len() * size_of::<T>();
        let count = (data_size + self.sector_size - 1) / self.sector_size;
        if lba >= self.total_sector || lba + count >= self.total_sector {
            return Err(VirtualDiskError::OutOfBounds);
        }
        let offset = lba * self.sector_size;
        unsafe {
            let p = self.vec.get_unchecked_mut(offset) as *mut u8;
            let q = data.get_unchecked(0) as *const _ as *const u8;
            p.copy_from(q, data_size);
        }
        Ok(data_size)
    }
}

#[derive(Debug)]
pub enum VirtualDiskError {
    OutOfBounds,
    IoError,
}
