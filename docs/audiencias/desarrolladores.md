# Óxido para desarrolladores

## Qué es hoy

Óxido es una herramienta de lenguaje en etapa temprana. `oxido-language` tokeniza y parsea un subconjunto estructural; la CLI exporta, compila y ejecuta ejemplos reales. El subconjunto documentado incluye items indentados, aliases de prelude y macros estándar, además de interoperabilidad Rust en módulos, métodos, referencias, `Result`/`Option`, `?` y colecciones. Los límites por característica están en la [matriz de compatibilidad](../compatibility.md).

## Usar la CLI instalada

En Debian o Ubuntu, instala el paquete desde [Instalación y primeros pasos](../instalacion.md). No necesitas clonar el repositorio para crear ni compilar un proyecto. Si el equipo no tiene un Rust compatible, instala la toolchain aislada una vez con `sudo oxido-instalar-toolchain`.

```sh
oxido nuevo mi-proyecto
cd mi-proyecto
oxido check src/main.ox
oxido build src/main.ox --release
oxido ejecutar src/main.ox
```

`verificar` usa `cargo check` sin crear un ejecutable. Para un proyecto `.ox`, `compilar` genera un proyecto Cargo en `target/oxido/`, conserva allí el Rust generado y los fuentes `.ox` (incluidos los `mod.ox`), y deja los binarios en `target/oxido/target/debug` o `target/oxido/target/release`; `--release` selecciona las optimizaciones de Cargo. `ejecutar` compila y ejecuta mediante Cargo; `probar` ejecuta las pruebas. Los alias `check`, `build`, `run` y `test` siguen disponibles.

## Desarrollar Óxido

Clonar el repositorio, compilar la CLI desde fuente, ejecutar las regresiones y enviar cambios son tareas de desarrollo del compilador. Están en la guía aparte [Clonar, compilar y contribuir](../contribuir.md); no son pasos necesarios para usar una Release instalada.
`exportar` crea además el mapa `.oxido-map.json` y resuelve módulos recursivos. `importar` conserva un único archivo tal cual por defecto; `--espanol` activa la conversión reversible. `importar-proyecto <directorio-rust> --salida <directorio-oxido>` recorre el árbol e importa cada `.rs` tal cual; con `--espanol` traduce el subconjunto y mantiene los items Rust no convertidos como passthrough. `#[path]` o construcciones especiales pueden requerir ajustes y no se garantiza roundtrip universal. `analizar-rust` valida archivos `.rs` directamente.

La CLI incluye `nuevo <directorio>`, `formato <archivo.ox> [--comprobar]` y `probar <archivo.ox>`. `formato` conserva comentarios e idempotencia en ambos modos; en modo indentado aplica las convenciones de layout de Óxido cubiertas en la matriz de [pruebas](../testing.md), sin cambiar el modo. El texto `.ox` no pretende ser idéntico al Rust: al exportar, el `.rs` final pasa por `rustfmt` 1.99.1 y coincide carácter por carácter con su salida para los fixtures publicados. La matriz cubre las producciones estructurales iniciales, no todas las expresiones posibles de Rust. `probar` delega en `cargo test`, por lo que un archivo sin funciones `#[test]` puede mostrar cero pruebas.

La CLI admite dependencias locales y versiones de crates.io mediante `oxido.toml`:

```toml
[language]
syntax = "indentation"
edition = "2024"

[dependencies.mi_crate]
version = "1.0"
```

Por defecto declara dependencias de crates.io con `version`. `oxido descargar-dependencias archivo.ox` muestra el diff del lockfile y las crates fijadas. Puede consultar el índice, pero no descarga paquetes ni guarda el lockfile; añade `--confirmar` después de revisar para ejecutar únicamente `cargo fetch --locked`. `--actualizar` prepara una nueva resolución y también requiere `--confirmar`. Una dependencia local `path` solo se acepta en esta acción con `--permitir-rutas-locales`; git y URL no están soportadas. `check`/`build`/`run` usan `--locked --offline`, requieren `Cargo.lock` para cualquier dependencia y fallan con `OX7001` si falta. Si hay dependencias, añade `--confiar-dependencias` solo después de revisar el proyecto: Cargo puede ejecutar `build.rs` y macros procedurales durante la compilación. Ese flag expresa confianza, no sandbox.

`oxido auditar-unsafe archivo.ox` informa tokens `inseguro`; añade `--rechazar` para que el comando falle al encontrar alguno. El análisis es léxico, no compila ni ejecuta el proyecto y no detecta toda la superficie de riesgo.

`oxido auditar-proyecto .` enumera `unsafe`, archivos `build.rs`, manifiestos de crates `proc-macro`, bloques FFI y llamadas `Command::new` detectables. No ejecuta Cargo ni sigue symlinks; los hallazgos usan rutas relativas. Es un inventario estático, no una garantía de seguridad.

El crate `oxido-lsp` habla LSP por stdio, publica diagnósticos léxicos y estructurales, semantic tokens, símbolos y, en workspace confiable, delega hover, completado, inlay hints, navegación y renombrado en `rust-analyzer`. La extensión de VS Code asocia `.ox`, declara Workspace Trust `limited` y ofrece `Óxido: Comprobar con Cargo`. Sigue siendo un alcance técnico: no hay publicación en marketplace, el VSIX no está firmado y la cobertura semántica no es universal. Consulta la [guía de VS Code y LSP](../vscode.md).

Para instalar la CLI y ejecutar el primer programa, consulta la [guía de instalación](../instalacion.md) y el [tutorial progresivo](../tutorial.md).

## Dirección técnica

- Rust 2024 es la edición objetivo.
- La semántica de tipos, ownership y borrowing pertenece a Rust.
- El frontend Óxido transforma estructura y aliases; tipos y expresiones conservan sintaxis Rust.
- Las macros estándar allowlisted traducen su nombre y pueden normalizar aliases en expresiones anidadas; DSLs de macros externos/no allowlisted y árboles de atributos desconocidos permanecen opacos.
- El backend genera con `quote`/`proc_macro2`, valida con Syn, usa `prettyplease` como formato intermedio y aplica `rustfmt` 1.99.1 al Rust final; los spans se mapean sobre la salida generada.

Consulta la [especificación](../language-spec.md), [arquitectura](../architecture.md), [pipeline](../compiler-pipeline.md) y [compatibilidad](../compatibility.md).

## Contribuir

Antes de abrir un cambio, lee [`CONTRIBUTING.md`](https://github.com/programadorkevinsvega-sys/oxido-lengue/blob/main/CONTRIBUTING.md). Añade casos positivos y negativos, ejecuta las pruebas del área y documenta cualquier límite que todavía no pueda probarse. Para orientarte, revisa las tareas abiertas del repositorio y la [arquitectura](../architecture.md).
