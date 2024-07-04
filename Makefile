.PHONY: love default all clean install iso run runs test apps doc kernel boot refresh
.SUFFIXES: .wasm

MNT			= ./mnt/
MISC		= ./misc/
ASSETS		= ./assets/

EFI_BOOT	= $(MNT)efi/boot
EFI_VENDOR	= $(MNT)efi/megos
KERNEL_BIN	= $(EFI_VENDOR)/kernel.bin
INITRD_IMG	= $(EFI_VENDOR)/initrd.img

KRNL_ARCH	= x86_64-unknown-none
TARGET_KERNEL	= system/target/$(KRNL_ARCH)/release/kernel.bin
TARGET_ISO	= var/megos.iso
ALL_TARGETS	= boot kernel apps
VAR_INITRD	= var/initrd/
INITRD_FILES	= LICENSE $(ASSETS)initrd/* apps/target/wasm32-unknown-unknown/release/*.wasm $(VAR_INITRD)*

SMP_X64			= system/src/arch/x64/smpinit
SMP_X64_ASM		= $(SMP_X64).asm
SMP_X64_BIN		= $(SMP_X64).bin

QEMU_X64		= qemu-system-x86_64
OVMF_X64		= var/ovmfx64.fd
EFI_CUSTOM_LOADER_X64	= var/bootx64.efi
BOOT_EFI_BOOT_X64	= $(EFI_BOOT)/bootx64.efi
BOOT_EFI_VENDOR_X64	= $(EFI_VENDOR)/bootx64.efi
TARGET_BOOT_EFI_X64	= boot/target/x86_64-unknown-uefi/release/boot-efi.efi

OVMF_X86		= var/ovmfia32.fd
EFI_CUSTOM_LOADER_X86	= var/bootx86.efi
BOOT_EFI_BOOT_X86	= $(EFI_BOOT)/bootia32.efi
BOOT_EFI_VENDOR_X86	= $(EFI_VENDOR)/bootia32.efi
TARGET_BOOT_EFI_X86	= boot/target/i686-unknown-uefi/release/boot-efi.efi

all: $(ALL_TARGETS)

default: all

clean:
	-rm -rf system/target apps/target boot/target tools/target lib/*/target

refresh: clean
	-rm system/Cargo.lock apps/Cargo.lock boot/Cargo.lock tools/Cargo.lock lib/*/Cargo.lock

# $(RUST_ARCH).json:
# 	rustc +nightly -Z unstable-options --print target-spec-json --target $(RUST_ARCH) | sed -e 's/-sse,+/+sse,-/' > $@

$(EFI_BOOT):
	mkdir -p $(EFI_BOOT)

$(EFI_VENDOR):
	mkdir -p $(EFI_VENDOR)

run:
	$(QEMU_X64) -machine q35 -cpu IvyBridge -smp 4,cores=2,threads=2 \
-bios $(OVMF_X64) \
-rtc base=localtime,clock=host \
-device virtio-net-pci \
-device nec-usb-xhci,id=xhci \
-device intel-hda -device hda-duplex \
-device usb-hub,bus=xhci.0,port=1,id=usb-hub \
-drive if=none,id=stick,format=raw,file=fat:rw:$(MNT) -device usb-storage,bus=xhci.0,port=2,drive=stick \
-device usb-tablet \
-device usb-audio,id=usb-audio \
-serial mon:stdio

run_up:
	$(QEMU_X64) -machine q35 -cpu IvyBridge \
-bios $(OVMF_X64) \
-rtc base=localtime,clock=host \
-device nec-usb-xhci,id=xhci -device usb-tablet \
-drive if=none,id=stick,format=raw,file=fat:rw:$(MNT) -device usb-storage,drive=stick \
-device intel-hda -device hda-duplex \
-monitor stdio

run_x86:
	$(QEMU_X64) -machine q35 -cpu IvyBridge -smp 4 \
-bios $(OVMF_X86) \
-rtc base=localtime,clock=host \
-device nec-usb-xhci,id=xhci -device usb-tablet \
-drive if=none,id=stick,format=raw,file=fat:rw:$(MNT) -device usb-storage,drive=stick \
-device intel-hda -device hda-duplex \
-monitor stdio

boot:
	(cd boot; cargo build --release --target x86_64-unknown-uefi --target i686-unknown-uefi)

kernel: $(SMP_X64_BIN)
	(cd system; cargo build --release --target $(KRNL_ARCH).json)

$(SMP_X64_BIN): $(SMP_X64_ASM)
	nasm -f bin $< -o $@

$(VAR_INITRD):
	-mkdir -p $(VAR_INITRD)

install: test $(EFI_VENDOR) $(EFI_BOOT) $(ALL_TARGETS) tools/mkinitrd/src/*.rs $(VAR_INITRD)
	if [ -f $(EFI_CUSTOM_LOADER_X64) ]; then cp $(EFI_CUSTOM_LOADER_X64) $(BOOT_EFI_BOOT_X64); \
		else cp $(TARGET_BOOT_EFI_X64) $(BOOT_EFI_BOOT_X64); fi
	cp $(TARGET_BOOT_EFI_X64) $(BOOT_EFI_VENDOR_X64)
	if [ -f $(EFI_CUSTOM_LOADER_X86) ]; then cp $(EFI_CUSTOM_LOADER_X86) $(BOOT_EFI_BOOT_X86); \
		else cp $(TARGET_BOOT_EFI_X86) $(BOOT_EFI_BOOT_X86); fi
	cp $(TARGET_BOOT_EFI_X86) $(BOOT_EFI_VENDOR_X86)
	cp $(TARGET_KERNEL) $(KERNEL_BIN)
	cargo run --manifest-path ./tools/mkinitrd/Cargo.toml -- -v $(INITRD_IMG) $(INITRD_FILES)

iso: install
	mkisofs -r -J -o $(TARGET_ISO) $(MNT)

apps:
	cd apps; cargo build --target wasm32-unknown-unknown --release
	for name in ./apps/target/wasm32-unknown-unknown/release/*.wasm; do \
	cargo run --manifest-path ./tools/wasm-strip/Cargo.toml -- -preserve name -strip-all $$name $$name; done

test:
	cargo test --manifest-path lib/megstd/Cargo.toml
	cargo test --manifest-path lib/meggl/Cargo.toml
	cargo test --manifest-path lib/wami/Cargo.toml
	cargo test --manifest-path lib/mar/Cargo.toml
	cargo test --manifest-path lib/uuid/Cargo.toml

doc:
	(cd system; cargo doc --all --target $(KRNL_ARCH).json)
