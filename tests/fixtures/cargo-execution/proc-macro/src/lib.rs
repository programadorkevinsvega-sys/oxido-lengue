use proc_macro::TokenStream;

#[proc_macro_attribute]
pub fn identity(_: TokenStream, item: TokenStream) -> TokenStream {
    if let Ok(path) = std::env::var("OXIDO_PROC_MARKER") {
        std::fs::write(path, "proc-macro").unwrap();
    }
    item
}
