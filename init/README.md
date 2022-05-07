# init

## What is this?

This is the first userspace program run by the kernel. Its goal is to make the
system usable by mounting drives, setting up the network interfaces, ... and
finally starting a graphical interface.

## Understanding the code architecture

The comments at the top of individual files are helpful. The entry point of the
program is the `main` function in `src/main.rs`. You should therefore start in
this file.

This is a quick look at the different modules:
```
main: The program entry point
|->config: Configuration specific to the machine
|->net: Set up networking
|->shutdown: Stuff to do before powering off the computer
|->sysctl: sysctl settings
|->ui: Start the Wayland compositor its udevd dependency
```

## How to build?

In order to reduce complexity and RAM usage, the program author prefers to
statically link the musl C library into the executable. Use this command to
build such an executable for the `x86_64` architecture:
```sh
cargo build --release --target x86_64-unknown-linux-musl
```
