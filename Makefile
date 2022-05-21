.PHONY: love default all clean install iso run runs test apps doc kernel boot
.SUFFIXED: .wasm


MNT			= ./mnt/
MISC		= ./misc/
ASSETS		= ./assets/

EFI_BOOT	= $(MNT)efi/boot
EFI_VENDOR	= $(MNT)efi/megos
KERNEL_BIN	= $(EFI_VENDOR)/kernel.bin
INITRD_IMG	= $(EFI_VENDOR)/initrd.img

KRNL_ARCH	= x86_64-unknown-none
TARGET_KERNEL	= system/target/$(KRNL_ARCH)/release/kernel
TARGET_ISO	= var/megos.iso
TARGETS		= boot kernel
ALL_TARGETS	= $(TARGETS) apps
INITRD_FILES	= LICENSE $(ASSETS)initrd/* apps/target/wasm32-unknown-unknown/release/*.wasm

OVMF_X64		= var/ovmfx64.fd
BOOT_EFI_BOOT1	= $(EFI_BOOT)/bootx64.efi
BOOT_EFI_VENDOR1	= $(EFI_VENDOR)/bootx64.efi
TARGET_BOOT_EFI1	= boot/target/x86_64-unknown-uefi/release/boot-efi.efi

OVMF_X86		= var/ovmfx86.fd
BOOT_EFI_BOOT2	= $(EFI_BOOT)/bootia32.efi
BOOT_EFI_VENDOR2	= $(EFI_VENDOR)/bootia32.efi
TARGET_BOOT_EFI2	= boot/target/i686-unknown-uefi/release/boot-efi.efi

default: $(TARGETS)

all: $(ALL_TARGETS)

clean:
	-rm -rf system/target apps/target boot/target tools/target

# $(RUST_ARCH).json:
# 	rustc +nightly -Z unstable-options --print target-spec-json --target $(RUST_ARCH) | sed -e 's/-sse,+/+sse,-/' > $@

$(EFI_BOOT):
	mkdir -p $(EFI_BOOT)

$(EFI_VENDOR):
	mkdir -p $(EFI_VENDOR)

run:
	qemu-system-x86_64 -machine q35 \
		-cpu Haswell -smp 4,cores=2,threads=2 \
		-bios $(OVMF_X64) \
		-rtc base=localtime,clock=host \
		-device nec-usb-xhci,id=xhci -device usb-tablet \
		-drive if=none,id=stick,format=raw,file=fat:rw:$(MNT) -device usb-storage,drive=stick \
		-device intel-hda -device hda-duplex \
		-monitor stdio

run_up:
	qemu-system-x86_64 -machine q35 \
		-cpu IvyBridge \
		-bios $(OVMF_X64) \
		-rtc base=localtime,clock=host \
		-device nec-usb-xhci,id=xhci -device usb-tablet \
		-drive if=none,id=stick,format=raw,file=fat:rw:$(MNT) -device usb-storage,drive=stick \
		-device intel-hda -device hda-duplex \
		-monitor stdio

run_x86:
	qemu-system-i386 -machine q35 \
		-cpu Haswell -smp 4,cores=2,threads=2 \
		-bios $(OVMF_X86) \
		-rtc base=localtime,clock=host \
		-device nec-usb-xhci,id=xhci -device usb-tablet \
		-drive if=none,id=stick,format=raw,file=fat:rw:$(MNT) -device usb-storage,drive=stick \
		-device intel-hda -device hda-duplex \
		-monitor stdio

boot:
	(cd boot; cargo build -Zbuild-std --release --target x86_64-unknown-uefi)
	(cd boot; cargo build -Zbuild-std --release --target i686-unknown-uefi)

kernel:
	(cd system; cargo build -Zbuild-std --release --target $(KRNL_ARCH).json)

install: test $(EFI_VENDOR) $(EFI_BOOT) $(ALL_TARGETS) tools/mkinitrd/src/*.rs
	cp $(TARGET_BOOT_EFI1) $(BOOT_EFI_BOOT1)
	cp $(TARGET_BOOT_EFI1) $(BOOT_EFI_VENDOR1)
	cp $(TARGET_BOOT_EFI2) $(BOOT_EFI_BOOT2)
	cp $(TARGET_BOOT_EFI2) $(BOOT_EFI_VENDOR2)
	cp $(TARGET_KERNEL) $(KERNEL_BIN)
	cargo run --manifest-path ./tools/mkinitrd/Cargo.toml -- $(INITRD_IMG) $(INITRD_FILES)

iso: install
	mkisofs -r -J -o $(TARGET_ISO) $(MNT)

apps:
	cd apps; cargo build --target wasm32-unknown-unknown --release
	for name in ./apps/target/wasm32-unknown-unknown/release/*.wasm; do \
	cargo run --manifest-path ./tools/wasm-strip/Cargo.toml -- -preserve name $$name $$name; done

test:
	cargo test --manifest-path lib/wasm/Cargo.toml

doc:
	(cd system; cargo doc --all --target $(KRNL_ARCH).json)
