pub fn version_line() -> String {
    format!("oxido {}", env!("CARGO_PKG_VERSION"))
}
