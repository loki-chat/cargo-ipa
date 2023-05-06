#[cfg(target_os = "macos")]
fn main() {}

#[cfg(not(target_os = "macos"))]
fn main() {
    panic!("cargo-ipa only supports macOS, and will only ever support macos.")
}
