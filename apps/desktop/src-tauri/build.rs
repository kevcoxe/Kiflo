fn main() {
    #[cfg(target_os = "macos")]
    link_macos_private_frameworks();

    tauri_build::build()
}

#[cfg(target_os = "macos")]
fn link_macos_private_frameworks() {
    if let Some(sdk_path) = sdk_path() {
        println!(
            "cargo:rustc-link-search=framework={}/System/Library/PrivateFrameworks",
            sdk_path
        );
    }
}

#[cfg(target_os = "macos")]
fn sdk_path() -> Option<String> {
    std::env::var("SDKROOT").ok().or_else(|| {
        std::process::Command::new("xcrun")
            .args(["--show-sdk-path"])
            .output()
            .ok()
            .filter(|output| output.status.success())
            .and_then(|output| String::from_utf8(output.stdout).ok())
            .map(|path| path.trim().to_string())
            .filter(|path| !path.is_empty())
    })
}
