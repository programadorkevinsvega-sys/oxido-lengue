use oxido_language::{
    SourceFile,
    domain::syntax::{LexOptions, SyntaxMode},
};

#[test]
fn translates_functions_around_qualified_macro_dsls_without_changing_tokens() {
    let rust = SourceFile::new(
        r#"pub async fn publish(correlation: &str) {
    tracing::error!(event="billing.catalog.outbox_invalid", component="commercial_billing", operation="publish_catalog_plan", dependency="outbox", correlation_id=%correlation, message=stringify!(si tipo verdadero));
}
"#,
    );

    let imported = oxido_compiler::import_rust(&rust)
        .expect("la función con macro externa debe superar la validación reversible");
    assert!(imported.contains("pub async funcion publish"));
    assert!(imported.contains(
        r#"tracing::error!(event="billing.catalog.outbox_invalid", component="commercial_billing", operation="publish_catalog_plan", dependency="outbox", correlation_id=%correlation, message=stringify!(si tipo verdadero));"#
    ));

    let options = LexOptions {
        project_mode: Some(SyntaxMode::Braces),
    };
    let generated = oxido_compiler::generate_ox(&SourceFile::new(imported), options)
        .expect("la sintaxis importada debe generar Rust");
    let original = oxido_rust_backend::format_for_roundtrip_comparison(
        &oxido_rust_backend::parse(&rust).expect("Rust de entrada válido"),
    )
    .expect("Rust original normalizado");
    let regenerated = oxido_rust_backend::format_for_roundtrip_comparison(
        &oxido_rust_backend::parse(&SourceFile::new(generated.rust))
            .expect("Rust regenerado válido"),
    )
    .expect("Rust regenerado normalizado");

    assert_eq!(
        regenerated, original,
        "la DSL de tracing debe conservar sus tokens"
    );
}
