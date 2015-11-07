#!/bin/sh

# Builds and runs tests for a particular target passed as an argument to this
# script.

set -ex

TARGET=$1
case "$TARGET" in
  *-apple-ios)
    # It's not possible to run a binary on the iOS simulator, so the best we can
    # do is compile. See https://github.com/rust-lang/rust/issues/29664.
    cargo rustc --verbose --target $TARGET -- -C link-args=-mios-simulator-version-min=7.0
    exit 0
    ;;

  *)
    cargo test --no-run --target $TARGET
    ;;
esac

TEST_BINARY=$(ls target/${TARGET}/debug/memmap-* | head -n1)

case "$TARGET" in
  arm-linux-androideabi)
    emulator @arm-18 -no-window &
    adb wait-for-device
    adb push ${TEST_BINARY} /data/memmap-test
    adb shell /data/memmap-test
    ;;

  arm-unknown-linux-gnueabihf)
    qemu-arm -L /usr/arm-linux-gnueabihf ${TEST_BINARY}
    ;;

  mips-unknown-linux-gnu)
    qemu-mips -L /usr/mips-linux-gnu ${TEST_BINARY}
    ;;

  aarch64-unknown-linux-gnu)
    qemu-aarch64 -L /usr/aarch64-linux-gnu/ ${TEST_BINARY}
    ;;

  *)
    ${TEST_BINARY}
    ;;
esac
