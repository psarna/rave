#!/usr/bin/env bash
set -euo pipefail

readonly BUILDROOT_VERSION="2026.05.1"
readonly BUILDROOT_SHA256="ae7f706f087b9ae9083a10a587368dfbf53103c28bf81c2d690198dc4090cb58"
readonly BUILDROOT_URL="https://buildroot.org/downloads/buildroot-${BUILDROOT_VERSION}.tar.xz"

repo_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
work_dir="${RAVE_LINUX_BUILD_DIR:-${repo_dir}/build/linux}"
download_dir="${RAVE_LINUX_DOWNLOAD_DIR:-${repo_dir}/build/downloads}"
source_dir="${repo_dir}/build/buildroot-${BUILDROOT_VERSION}"
archive="${download_dir}/buildroot-${BUILDROOT_VERSION}.tar.xz"
jobs="${JOBS:-}"

# Buildroot rejects LD_LIBRARY_PATH entries for the current directory, and its
# host tools should not depend on libraries injected by the calling shell.
unset LD_LIBRARY_PATH

usage() {
    cat <<EOF
Build a minimal RV64IMAC Linux guest for rave with Buildroot.

Usage: $0 [--help]

Environment variables:
  JOBS                       Parallel build jobs (default: detected CPU count)
  RAVE_LINUX_BUILD_DIR       Buildroot output directory (default: build/linux)
  RAVE_LINUX_DOWNLOAD_DIR    Persistent download cache (default: build/downloads)

Artifacts:
  <build-dir>/images/Image
  <build-dir>/images/rootfs.cpio
EOF
}

while (($#)); do
    case "$1" in
        -h|--help) usage; exit 0 ;;
        *) echo "unknown argument: $1" >&2; usage >&2; exit 2 ;;
    esac
    shift
done

for command in curl make sha256sum tar awk; do
    if ! command -v "$command" >/dev/null 2>&1; then
        echo "missing required command: $command" >&2
        exit 1
    fi
done

if [[ -z "$jobs" ]]; then
    if command -v nproc >/dev/null 2>&1; then
        jobs="$(nproc)"
    elif command -v getconf >/dev/null 2>&1; then
        jobs="$(getconf _NPROCESSORS_ONLN)"
    else
        jobs=1
    fi
fi

if [[ ! "$jobs" =~ ^[1-9][0-9]*$ ]]; then
    echo "JOBS must be a positive integer, got: $jobs" >&2
    exit 2
fi

mkdir -p "$download_dir" "$(dirname "$work_dir")"

if [[ ! -f "$archive" ]]; then
    echo "Downloading Buildroot ${BUILDROOT_VERSION}..."
    curl --fail --location --retry 3 --output "${archive}.part" "$BUILDROOT_URL"
    mv "${archive}.part" "$archive"
fi

actual_sha256="$(sha256sum "$archive" | awk '{print $1}')"
if [[ "$actual_sha256" != "$BUILDROOT_SHA256" ]]; then
    echo "Buildroot archive checksum mismatch" >&2
    echo "expected: $BUILDROOT_SHA256" >&2
    echo "actual:   $actual_sha256" >&2
    exit 1
fi

if [[ ! -f "$source_dir/Makefile" ]]; then
    echo "Extracting Buildroot ${BUILDROOT_VERSION}..."
    tar -xf "$archive" -C "$(dirname "$source_dir")"
fi

echo "Configuring a static RV64IMAC/LP64 BusyBox system..."
make -C "$source_dir" \
    O="$work_dir" \
    BR2_EXTERNAL="$repo_dir/buildroot" \
    rave_defconfig

echo "Building Linux and rootfs.cpio with $jobs jobs..."
make -C "$source_dir" \
    O="$work_dir" \
    BR2_EXTERNAL="$repo_dir/buildroot" \
    BR2_DL_DIR="$download_dir" \
    -j"$jobs"

kernel="$work_dir/images/Image"
initrd="$work_dir/images/rootfs.cpio"

if [[ ! -s "$kernel" || ! -s "$initrd" ]]; then
    echo "Build completed without the expected Image and rootfs.cpio" >&2
    exit 1
fi

echo
echo "Built rave Linux guest:"
echo "  kernel: $kernel"
echo "  initrd: $initrd"
echo
echo "Boot it with:"
printf '  cargo run --release -- boot --firmware demo/fw_jump.bin --kernel %q --initrd %q --dtb demo/rave.dtb --memory 128M\n' \
    "$kernel" "$initrd"

