# cargo-ipa
A cargo subcommand for compiling .ipa and .app files, for iOS and macOS (respectively).

cargo-ipa **exclusively** supports macOS, and will only ever support macOS. Because Apple is Apple, you can't compile iOS apps on non-mac systems (doing so requires the iOS SDK, which is closed source and only on macOS).

# Installation
cargo-ipa isn't in a "finished" state yet, so it's not on crates.io. You can, however, still install from Git:

`cargo install --git https://github.com/loki-chat/cargo-ipa.git`

To install with swift-bridge integration:

`cargo install --git https://github.com/loki-chat/cargo-ipa.git --feature swift-bridge`

# Usage
Currently, cargo-ipa can only build unsigned IPA and .app files. Hopefully, in the future, it'll support signing and installing IPAs as well.

## Building IPAs & apps
For binary projects, just run `cargo ipa build`. For library examples, run `cargo ipa build -e <example_name>` (or `--example` instead of `-e`).

By default, cargo-ipa will make 4 files: an IPA for x86_64 iOS, an IPA for aarch64 iOS, an app for x86_64 macOS, and an app for aarch64 macOS. You can limit these with the `-p`/`--platform` and `-a`/`--architecture` flags; you can set the platform to just iOS or just macOS, and the architecture to just x86_64 or just aarch64 devices. For example, to compile your cool app for M1 (and later) macs, you could run:

`cargo ipa build --platform macOS --architecture aarch64`

(or, if you're a normal person and find architecture impossible to spell: `cargo ipa build -p macOS -a aarch64`.)

## App Name
In the `Info.plist`, Apple requires both an app name (as an ID, eg "my-app"), and a human readable name (eg "My App"). cargo-ipa will set the ID to the package name in `Cargo.toml`, but needs a human readable name. You can either set this via the `name` setting (see [Configuration](#configuration)), or pass the `-n` (or `--name`) argument to `cargo-ipa`.

# Configuration
cargo-ipa reads settings directly from your `Cargo.toml`. Simply add a `package.metadata.cargo-ipa` section in your `Cargo.toml`, and it'll read all the settings from there. For example, to set your app's name, you could add this to your `Cargo.toml`:

```toml
[package.metadata.cargo-ipa]
name = "My App"
```

## Info.plist Overrides
Every macOS/iOS app has an `Info.plist` file. By defualt, cargo-ipa will automatically set these settings in the `Info.plist`:

- `CFBundleExecutable`: This is the name of the executable in the app. This gets set to the project's name (or library example's name, if you're compiling an example).
- `CFBundleIdentifier`: This is the bundle identifier for the app. By default, cargo-ipa sets this to `com.<binary-name>`, where binary name is the project's name or library example's name.
- `CFBundleName`: This is a human-readable bundle identifier, and what appears as the app's name on the device's home screen/app list. cargo-ipa will load this from the `-n`/`--name` argument, or the name setting in your [configuration](#configuration).
- `CFBundleVersion`: This is the app's version. cargo-ipa will load this from the version listed in your `Cargo.toml`.
- `CFBundleShortVersionString`: This is basically the same as above, but requires a `<major version>.<minor version>.<patch version>` format. cargo-ipa will load this from `Cargo.toml` just like above; this can lead to issues if the `Cargo.toml` version is not in the correct format, and in the future, cargo-ipa should be able to convert your `Cargo.toml` version into that format, so it's in the valid.

cargo-ipa won't set any other settings in the `Info.plist`. To set (or override) more settings in the `Info.plist`, you can use the `properties` section of cargo-ipa's [configuration](#configuration), like so:

```toml
[package.metadata.cargo-ipa.properties]
CFBundleShortVersionString = 0.1.0
```

# Swift-bridge integration
Since many Apple APIs still rely on Swift code, cargo-ipa can integrate with [swift-bridge](https://github.com/chinedufn/swift-bridge/tree/master) to compile Swift and Rust together. To use it, you need to install cargo-ipa with the swift-bridge feature, and then configure the `swift-library` and `swift-bridges` settings.

`swift-library` is literally just the folder that has your library in it; for example, if your project has `my-swift-library/Sources/my-swift-library` in it, then set `swift-library = "my-swift-library` in your Cargo.toml.

`swift-bridges` is a list of Rust files that actually use swift-bridge's FFI (via the `#[swift_bridge::bridge]` macro). These are indexed from the project root (the folder that houses `Cargo.toml`). For example, if you have a file called `swift.rs` that handles FFI, your setting will probably look like this: `swift-bridges = ["src/swift.rs"]`.

# Complete list of settings
- `name`: A string representing the app's name, as it appears in the app list or on the home screen. See [App Name](#app-name).
- `properties`: A table of keys/values to put in the `Info.plist` file. See [Info.plist Overrides](#infoplist-overrides)
- `swift-bridges`: A list of Rust files to compile using [swift-bridge](https://github.com/chinedufn/swift-bridge/tree/master). See [Swift-bridge integration](#swift-bridge-integration).
- `swift-library`: The Swift package to compile using [swift-bridge](https://github.com/chinedufn/swift-bridge/tree/master). See [Swift-bridge integration](#swift-bridge-integration).

Here's an example of all the cargo-ipa settings:

```toml
[package]
name = "my-app"
description = "My fancy app, compiled for iOS with cargo-ipa"
authors = ["Me"]
version = "0.1.0"
edition = "2021"
license = "MIT OR Apache-2.0"
repository = "https://github.com/loki-chat/cargo-ipa"

[package.metadata.cargo-ipa]
name = "My App"
swift-bridges = ["src/swift.rs"]
swift-library = "swift-library"

[package.metadata.cargo-ipa.properties]
MinimumOSVersion = "14.0.0"
```


# Signing IPAs

Currently, cargo-ipa doesn't support signing the IPA files. It'll just spit out a raw, unsigned IPA file, which you can sign using any existing IPA signer. IPA will hopefully be added in the future.
