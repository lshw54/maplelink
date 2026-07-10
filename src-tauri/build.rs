fn main() {
    // Re-embed the Windows exe / window icon whenever it changes. Without this,
    // an incremental build that only sees the new binary icon files keeps the
    // old icon compiled into the executable.
    println!("cargo:rerun-if-changed=icons/icon.ico");
    tauri_build::build();
}
