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

# Verify if debug and release builds coexist
if [ -e target/x86_64/debug/silicium ] && [ -e target/x86_64/release/silicium ]; then
    # Copy the most recent build
    if [ target/x86_64/debug/silicium -nt target/x86_64/release/silicium ]; then
        cp -v target/x86_64/debug/silicium iso/boot/silicium.elf
    else
        cp -v target/x86_64/release/silicium iso/boot/silicium.elf
    fi
elif [ -e target/x86_64/debug/silicium ]; then
    cp -v target/x86_64/debug/silicium iso/silicium.elf
elif [ -e target/x86_64/release/silicium ]; then
    cp -v target/x86_64/release/silicium iso/silicium.elf
else
    die "No kernel executable found"
fi

# Create the ISO
xorriso -as mkisofs -b boot/limine-cd.bin                   \
        -no-emul-boot -boot-load-size 4 -boot-info-table 	\
        --efi-boot boot/limine-cd-efi.bin 					\
        -efi-boot-part --efi-boot-image  					\
        --protective-msdos-label iso -o bin/silicium.iso
    ./bin/src/limine/limine-deploy bin/silicium.iso
