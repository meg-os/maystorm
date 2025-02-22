# MEG-OS

![GitHub](https://img.shields.io/github/license/neri/maystorm) ![GitHub top language](https://img.shields.io/github/languages/top/neri/maystorm)

My first hobby operating system.

## Feature

* A hobby operating system written in Rust
* Not a POSIX clone system
  * Designed for use by a single user
* Supports applications in WebAssembly format

## Requirements

* Platform: IBM PC Compatibles
* Processor: x64 processor with up to 64 cores
* RAM: ??? GB
* Storage: ???
* Display: 800 x 600

## Build Environment

* Rust nightly
  * `rustup target add wasm32-unknown-unknown`
* nasm
* qemu + ovmf (optional)

### Minimum supported Rust version

The latest version is recommended whenever possible.

### building

1. `make install`

### run on qemu

1. Follow the build instructions to finish the installation.
2. Copy qemu's OVMF for x64 to `var/ovmfx64.fd`.
3. `make run`

### run on real hardware

1. Follow the build instructions to finish the installation.
2.  Copy the files in the path `mnt/efi` created by the build to a USB memory stick and reboot your computer.
* You may need to change settings such as SecureBoot.

## HOE: Haribote-OS Emulation Subsystem

* Some uncompressed Haribote-OS apps will work; some apps may not work due to different basic OS behaviour.
* This subsystem may be replaced by another implementation in the future.

## History

### 2020-05-09

* Initial Commit

## LICENSE

MIT License

&copy; 2020-2024 MEG-OS Project.
