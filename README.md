# cargo-ipa

A cargo subcommand for generating IPA files for iOS apps.

# Installation

cargo-ipa isn't in a "finished" state yet, so it's not on crates.io. You can, however, still install from Git:
`cargo install --git https://github.com/loki-chat/cargo-ipa.git`

# Usage

Currently, cargo-ipa can only build unsigned IPA files. Hopefully, in the future, it'll support signing and installing IPAs as well.

## Building IPAs

For binary projects, just run `cargo ipa build`. For libraries, run `cargo ipa -e <example_name>` (or `--example` instead of `-e`) to compile a library example.

## App Name

In the `Info.plist`, Apple requires both an app name (as an ID, eg "my-app"), and a human readable name (eg "My App"). cargo-ipa will set the ID to the package name in `Cargo.toml`, but needs a human readable name. You can either set this via an Info.plist override (see below), or pass the `-n` (or `--name`) argument to `cargo-ipa`.

## Info.plist Overrides

cargo-ipa will load values for your app's `Info.plist` directly from `Cargo.toml`. Just add a `cargo-ipa` section, and declare your values under that. For example, to set the `CFBundleName` (app name) property, you'd add this to your `Cargo.toml`:

```
[cargo-ipa]
CFBundleName = "My App"
```

If you set `CFBundleName` in `Cargo.toml`, you won't need to pass the `-n` argument to `cargo-ipa`.

## Signing IPAs

Currently, cargo-ipa doesn't support signing the IPA files. It'll just spit out a raw, unsigned IPA file; you can sign this using an external program. cargo-ipa should have signing support soon™.
