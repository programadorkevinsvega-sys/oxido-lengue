fn main() {
    if let Ok(path) = std::env::var("OXIDO_BUILD_MARKER") {
        std::fs::write(path, "build").unwrap();
    }
    println!("cargo:warning=fixture build script ran");
}
