use proc_macro::{Literal, TokenStream, TokenTree};

#[proc_macro_attribute]
pub fn identity(_: TokenStream, item: TokenStream) -> TokenStream {
    if let Ok(path) = std::env::var("OXIDO_PROC_MARKER") {
        std::fs::write(path, "proc-macro").unwrap();
    }
    item
}

#[proc_macro]
pub fn token_text(input: TokenStream) -> TokenStream {
    TokenStream::from(TokenTree::Literal(Literal::string(&input.to_string())))
}
