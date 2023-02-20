#!/bin/sh
set -e
die() {
    echo "error: $@" >&2
    exit 1
}

[ -e ./README.md ]   \
    || die "you must run this script from the root of the repository"

qemu-system-x86_64 -m 128                                   \
    -drive format=raw,media=cdrom,file=bin/silicium-rs.iso  \
    -no-reboot                                              \
    -no-shutdown                                            \
    -serial stdio                                           \
    -smp 4
