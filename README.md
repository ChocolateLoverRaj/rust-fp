# `rust-fp`
A better fingerprint library than `libfprint`

## Why
- `libfprint` seems to not support fingerprint readers with their own built-in matching
- `libfprint` is written in C, not Rust
- `libfprint` is hard to use

## Goals
- Support enrolling fingerprints through a GUI and CLI
- Support authenticating with fingerprints with PAM
- Be easy to develop new drivers, even if you just have 1 test device and it's the same device that you're using to code
- Support Chromebook fingerprint sensors
- Be modular and easy to use in non-Linux environments (such as RedoxOS)
- Be easy to set up a development environment to build and edit the code
- Provide high quality auto-complete in code editors
- Be as close to 100% Rust as possible with minimal non-Rust dependencies

## Status
### Drivers
Currently, `rust-fp` is not yet written. It will eventually support Chromebook fingerprint readers, and other people can add drivers for their own fp sensors.

### Integration with desktop environments
None right now. KDE should be able to be integrated with a hack. I'm planning on integrating this with COSMIC.
