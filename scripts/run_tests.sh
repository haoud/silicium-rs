 #!/bin/sh
set -e
die() {
    echo "error: $@" >&2
    exit 1
}

[ -e ./README.md ]   \
    || die "you must run this script from the root of the repository"

# Run the "normal" tests (i.e on the same machine)
cargo +nightly test -p silicium-x86_64 --target=x86_64-unknown-linux-gnu -Z build-std

# Run the "cross" tests (i.e trough QEMU)
# ...