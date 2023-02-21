#!/bin/sh
set -e
die() {
    echo "error: $@" >&2
    exit 1
}

[ -e ./README.md ]   \
    || die "you must run this script from the root of the repository"

# Copy the limine bootloader
cp -v                                   \
    bin/src/limine/limine-cd-efi.bin    \
    bin/src/limine/limine-cd.bin        \
    bin/src/limine/limine.sys           \
    iso/boot/

# Copy the kernel
cp target/x86_64/debug/silicium iso/boot/silicium.elf

# Create the ISO
xorriso -as mkisofs -b boot/limine-cd.bin                   \
        -no-emul-boot -boot-load-size 4 -boot-info-table 	\
        --efi-boot boot/limine-cd-efi.bin 					\
        -efi-boot-part --efi-boot-image  					\
        --protective-msdos-label iso -o bin/silicium.iso
    ./bin/src/limine/limine-deploy bin/silicium.iso
