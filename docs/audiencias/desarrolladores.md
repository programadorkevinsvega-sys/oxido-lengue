# Óxido para desarrolladores

## Qué es hoy

Óxido es una herramienta de lenguaje en etapa temprana. `oxido-language` tokeniza y parsea un subconjunto estructural; la CLI exporta, compila y ejecuta ejemplos reales. Fases 7–8 incluyen items indentados, aliases de prelude y macros estándar, además de interoperabilidad Rust en módulos, métodos, referencias, `Result`/`Option`, `?` y colecciones. Los límites por característica están en la matriz de compatibilidad.

Flujo disponible desde la raíz del repositorio:

```sh
cargo run --locked -p oxido-cli -- exportar examples/hola_mundo.ox --salida target/hola.rs
cargo run --locked -p oxido-cli -- compilar examples/hola_mundo.ox
cargo run --locked -p oxido-cli -- ejecutar examples/hola_mundo.ox
cargo run --locked -p oxido-cli -- ejecutar examples/fase_7_estructuras.ox
cargo run --locked -p oxido-cli -- ejecutar examples/fase_7_interoperabilidad.ox
```

Para generar y ejecutar con las optimizaciones del perfil release de Cargo, añade `--release` a `compilar` o `ejecutar`. Por ejemplo: `cargo run --locked -p oxido-cli -- ejecutar examples/hola_mundo.ox --release`.

`exportar` crea además el mapa `.oxido-map.json` y resuelve módulos recursivos. `importar` convierte un único archivo dentro del subconjunto reversible. `importar-proyecto <directorio-rust> --salida <directorio-oxido>` recorre el árbol y mantiene los items Rust no convertidos como passthrough; `#[path]` o construcciones especiales pueden requerir ajustes y no se garantiza roundtrip universal. `analizar-rust` valida archivos `.rs` directamente.

La CLI incluye `nuevo <directorio>`, `formato <archivo.ox> [--comprobar]` y `probar <archivo.ox>`. `formato` es un prototipo: reimporta como modo llaves y no conserva fuentes indentadas; no lo uses para reformatear esas fuentes. `probar` delega en `cargo test`, por lo que un archivo sin funciones `#[test]` puede mostrar cero pruebas.

La CLI admite dependencias locales y versiones de crates.io mediante `oxido.toml`:

```toml
[language]
syntax = "indentation"
edition = "2024"

[dependencies.mi_crate]
version = "1.0"
```

Por defecto declara dependencias de crates.io con `version`. `oxido descargar-dependencias archivo.ox` muestra el diff del lockfile y las crates fijadas. Puede consultar el índice, pero no descarga paquetes ni guarda el lockfile; añade `--confirmar` después de revisar para ejecutar únicamente `cargo fetch --locked`. `--actualizar` prepara una nueva resolución y también requiere `--confirmar`. Una dependencia local `path` solo se acepta en esta acción con `--permitir-rutas-locales`; git y URL no están soportadas. `compilar`/`ejecutar` usan `--locked --offline`, requieren `Cargo.lock` para cualquier dependencia y fallan con `OX7001` si falta. Si hay dependencias, añade `--confiar-dependencias` solo después de revisar el proyecto: Cargo puede ejecutar `build.rs` y macros procedurales durante la compilación. Ese flag expresa confianza, no sandbox.

`oxido auditar-unsafe archivo.ox` informa tokens `inseguro`; añade `--rechazar` para que el comando falle al encontrar alguno. El análisis es léxico, no compila ni ejecuta el proyecto y no detecta toda la superficie de riesgo.

`oxido auditar-proyecto .` enumera `unsafe`, archivos `build.rs`, manifiestos de crates `proc-macro`, bloques FFI y llamadas `Command::new` detectables. No ejecuta Cargo ni sigue symlinks; los hallazgos usan rutas relativas. Es un inventario estático, no una garantía de seguridad.

El crate `oxido-lsp` es un prototipo stdio con diagnósticos léxicos básicos. La respuesta de semantic tokens es heurística e incompleta, no sirve para resaltado fiable; tampoco integra `rust-analyzer`, Workspace Trust ni compilación desde VS Code. No hay una extensión VS Code funcional.

## Dirección técnica

- Rust 2024 es la edición objetivo.
- La semántica de tipos, ownership y borrowing pertenece a Rust.
- El frontend Óxido transforma estructura y aliases; tipos y expresiones conservan sintaxis Rust.
- Las macros estándar allowlisted traducen su nombre y pueden normalizar aliases en expresiones anidadas; DSLs de macros externos/no allowlisted y árboles de atributos desconocidos permanecen opacos.
- El backend genera con `quote`/`proc_macro2` y formatea con `prettyplease`; los spans se guardan en un mapa lateral.

Consulta la [especificación](/referencia/language-spec/), [arquitectura](/referencia/arquitectura/), [pipeline](/referencia/compiler-pipeline/) y [compatibilidad](/referencia/compatibility/).

## Contribuir

Trabaja desde una fase del [roadmap](/referencia/roadmap/), añade casos positivos y negativos, y documenta cualquier límite que aún no pueda probarse.
