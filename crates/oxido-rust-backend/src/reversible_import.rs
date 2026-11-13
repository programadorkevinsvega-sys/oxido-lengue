use oxido_language::{
    SourceFile,
    domain::syntax::{LexOptions, SyntaxMode, lex},
};
use quote::ToTokens;
use syn::{parse::Error as SynError, spanned::Spanned};

/// Importa funciones Rust como `funcion` sin reescribir sus cuerpos.
///
/// Las macros con ruta permanecen como grupos opacos. La capa de aplicación
/// valida luego que exportar el `.ox` conserve todos los tokens de Rust.
pub fn import_reversible(source: &SourceFile) -> Result<String, SynError> {
    let file = syn::parse_file(source.text())?;
    let lexed = lex(
        source,
        LexOptions {
            project_mode: Some(SyntaxMode::Braces),
        },
    );
    let mut replacements = Vec::new();
    for item in &file.items {
        let syn::Item::Fn(function) = item else {
            continue;
        };
        let requires_attribute_passthrough = function.attrs.iter().any(|attribute| {
            !attribute.path().is_ident("doc")
                && (attribute.path().segments.len() > 1
                    || !matches!(attribute.meta, syn::Meta::Path(_)))
        });
        if function.sig.constness.is_some()
            || function.sig.unsafety.is_some()
            || function.sig.abi.is_some()
            || function.sig.variadic.is_some()
            || requires_attribute_passthrough
            || function
                .block
                .to_token_stream()
                .to_string()
                .contains("let [")
        {
            continue;
        }
        let expected = function.sig.fn_token.span().start();
        let keyword = lexed.tokens.iter().find(|token| {
            source.slice(token.span) == "fn"
                && source.position(token.span.start()).is_some_and(|position| {
                    position.line == expected.line
                        && position.column.saturating_sub(1) == expected.column
                })
        });
        let Some(keyword) = keyword else {
            return Err(SynError::new(
                function.sig.fn_token.span(),
                "no se pudo localizar la palabra `fn` en el archivo fuente para importar sin perder trivia",
            ));
        };
        replacements.push((keyword.span.start(), keyword.span.end()));
    }

    replacements.sort_unstable_by_key(|(start, _)| *start);
    let mut output = String::from("sintaxis llaves\n");
    let mut cursor = 0;
    for (start, end) in replacements {
        output.push_str(&source.text()[cursor..start]);
        output.push_str("funcion");
        cursor = end;
    }
    output.push_str(&source.text()[cursor..]);
    Ok(output)
}
