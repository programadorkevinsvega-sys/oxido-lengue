#![forbid(unsafe_code)]

//! Adaptador que valida y formatea archivos Rust mediante AST.

use oxido_language::SourceFile;
use syn::parse::Error as SynError;

/// Rust parseado; la representación concreta de `syn` queda confinada al adapter.
pub struct RustFile(syn::File);

/// Analiza una fuente completa como archivo Rust.
pub fn parse(source: &SourceFile) -> Result<RustFile, SynError> {
    syn::parse_file(source.text()).map(RustFile)
}

/// Formatea un AST Rust sin concatenar o interpolar texto de entrada.
#[must_use]
pub fn format(file: &RustFile) -> String {
    prettyplease::unparse(&file.0)
}

impl RustFile {
    /// Cantidad de items top-level presentes en el archivo.
    #[must_use]
    pub fn item_count(&self) -> usize {
        self.0.items.len()
    }
}

/// Valida un fragmento de expresión Rust sin analizar macros expandidas ni semántica.
pub fn validate_expression(fragment: &str) -> Result<(), SynError> {
    syn::parse_str::<syn::Expr>(fragment).map(|_| ())
}

/// Valida una cabecera `for` Rust conservándola como tokens.
pub fn validate_for_header(fragment: &str) -> Result<(), SynError> {
    let header = fragment
        .parse::<proc_macro2::TokenStream>()
        .map_err(|error| SynError::new(proc_macro2::Span::call_site(), error.to_string()))?;
    let expression = quote::quote! { for #header {} };
    syn::parse2::<syn::Expr>(expression).map(|_| ())
}

/// Valida un fragmento de tipo Rust.
pub fn validate_type(fragment: &str) -> Result<(), SynError> {
    syn::parse_str::<syn::Type>(fragment).map(|_| ())
}

/// Valida un fragmento de patrón Rust.
pub fn validate_pattern(fragment: &str) -> Result<(), SynError> {
    use syn::parse::Parser;
    syn::Pat::parse_single.parse_str(fragment).map(|_| ())
}

/// Valida la firma de una función preservada por el AST.
pub fn validate_signature(fragment: &str) -> Result<(), SynError> {
    let item = format!("fn {fragment}");
    syn::parse_str::<syn::Signature>(&item).map(|_| ())
}

/// Valida un item Rust passthrough completo.
pub fn validate_item(fragment: &str) -> Result<(), SynError> {
    syn::parse_str::<syn::File>(fragment).map(|_| ())
}
