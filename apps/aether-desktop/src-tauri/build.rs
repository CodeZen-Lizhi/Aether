fn main() {
    println!(
        "cargo:rustc-env=AETHER_DESKTOP_TARGET={}",
        std::env::var("TARGET").expect("Cargo target")
    );
    tauri_build::build();
}
