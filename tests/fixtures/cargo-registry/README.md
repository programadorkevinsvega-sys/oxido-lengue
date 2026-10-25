# Fixture de crates.io para pruebas Cargo

Este fixture es el registro local de integración de Cargo. Cubre una crate de formato numérico, una dependencia async y las macros procedurales de bibliotecas existentes:

- `itoa` **1.0.18**, MIT/Apache-2.0.
- `futures-executor` **0.3.34**, MIT/Apache-2.0; sus transitivas también están fijadas en `Cargo.lock`.
- `paste` **1.0.15**, MIT/Apache-2.0, macro procedural funcional que transforma tokens en un identificador de función.
- `serde`/`serde_derive` **1.0.229**, MIT/Apache-2.0, para `Serialize` y `Deserialize`.
- `serde_json` **1.0.154**, MIT/Apache-2.0, incluida su macro `json!` con DSL.
- `tokio` **1.54.1** y `tokio-macros` **2.7.2**, MIT; cubren el atributo `tokio::main` y `tokio::select!`.

El manifiesto y lockfile describen el paquete temporal `oxido_generated` que emite el compilador. El contenido de `vendor/` se generó desde crates.io con Cargo, no se editó manualmente:

```sh
cargo generate-lockfile
cargo vendor --locked vendor
```

La prueba apunta un `CARGO_HOME` temporal a este directorio mediante `source.crates-io.replace-with`, de modo que no consulta la caché del desarrollador ni necesita red. Los checksums de los paquetes en `Cargo.lock` se comparan con `.cargo-checksum.json` antes de aceptar el resultado.

Las pruebas CLI copian el `Cargo.lock` a un proyecto temporal, usan un `CARGO_HOME` temporal sin caché de crates y configurado para este vendor, y ejecutan `oxido descargar-dependencias`, compilación `--locked --offline`, ejecución y compilación/ejecución independiente del árbol exportado. El caso macro comprueba `serde` derive, `#[serde(rename = ...)]`, el atributo `tokio::main(flavor = "current_thread")`, `tokio::select!`, `serde_json::json!` y `paste::paste!`. `paste` transforma `[<macro_ si>]` en una función `macro_si`, que luego se invoca y ejecuta; su cuerpo usa `stringify!` para verificar identificadores, palabras que son aliases de Óxido, cadena y delimitadores anidados. También prueba módulos anidados mixtos `.ox`/`.rs` con `#[path]`. Una entrada inválida a `json!` comprueba el diagnóstico de rustc relacionado con el `.ox` al compilar mediante Óxido.

Este fixture prueba versiones concretas; no representa compatibilidad general con todas las versiones o macros de esas crates. No contiene `build.rs` ni una macro procedural cuyo propósito sea ejecutar efectos laterales. Esos riesgos se prueban por separado en `tests/fixtures/cargo-execution/` y requieren confianza explícita al compilar.
