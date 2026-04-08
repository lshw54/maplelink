fn main() {
    // Only embed requireAdministrator manifest in release builds.
    // Dev mode should run without elevation for convenience.
    #[cfg(target_os = "windows")]
    {
        let profile = std::env::var("PROFILE").unwrap_or_default();
        if profile == "release" {
            let res = tauri_build::WindowsAttributes::new()
                .app_manifest(include_str!("maplelink.manifest"));
            let attrs = tauri_build::Attributes::new().windows_attributes(res);
            tauri_build::try_build(attrs).expect("failed to run tauri build");
        } else {
            tauri_build::build();
        }
    }

    #[cfg(not(target_os = "windows"))]
    {
        tauri_build::build();
    }
}
