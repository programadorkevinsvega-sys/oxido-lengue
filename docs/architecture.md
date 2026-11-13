# Arquitectura de Óxido

El contrato normativo está en [ARCHITECTURE_RULES.md](../ARCHITECTURE_RULES.md). La tabla refleja los paquetes del workspace declarados en `Cargo.toml` y los componentes editoriales del árbol actual.

El parser propio se limita a la estructura Óxido y preserva los fragmentos Rust. El objetivo normativo de interoperabilidad amplia con Rust, sin modificar bibliotecas externas, está descrito en el [pipeline del compilador](compiler-pipeline.md#contrato-de-interoperabilidad-con-rust). La cobertura incompleta es una brecha de implementación y pruebas, no un límite permanente del lenguaje.

## Componentes

| Componente | Responsabilidad | Tipo de módulo |
|---|---|---|
| `oxido-language` | Fuente/spans UTF-8, diagnósticos, lexer, AST estructural y parser de `.ox` | Núcleo/dominio |
| `oxido-compiler` | Casos de uso de parseo, generación, mapa, integración Cargo e importación reversible | Aplicación |
| `oxido-rust-backend` | Emitir Rust con quote/proc_macro2; validar con Syn, formato intermedio prettyplease y formato final rustfmt 1.99.1 | Adapter de salida |
| `oxido-cli` | `aliases`, `analizar-rust`, `auditar-proyecto`, `auditar-unsafe`, `compilar`, `descargar-dependencias`, `ejecutar`, `exportar`, `formato`, `importar`, `importar-proyecto`, `nuevo` y `probar` | Adapter de entrada |
| `oxido-cargo` | Invocar Cargo por argv, limitar tiempo y leer mensajes JSON | Adapter de proceso |
| `oxido-lsp` | Transporte stdio, análisis compartido, diagnósticos, semantic tokens, bridge persistente a `rust-analyzer` y comprobación Cargo explícita | Servidor funcional; alcance y límites probados en la [guía de VS Code y LSP](vscode.md) |
| `oxido-benchmark` | Harnesses de comparación de rendimiento documentados | Herramienta del workspace |
| `oxido-playground` | Servicio local de laboratorio con ejecución aislada en Linux | Herramienta disponible; no desplegada como servicio público |
| `editors/vscode-oxido` | Extensión VS Code para `.ox`, configuración, comandos y Workspace Trust | Cliente funcional en desarrollo; evidencia y límites en [VS Code y LSP](vscode.md) |
| `website` | Portal estático Markdown con documentación por audiencia y referencia | Frontend documental; Astro Starlight |

La lista de paquetes Rust se puede reproducir con `cargo metadata --format-version 1 --no-deps`; el árbol `crates/`, `tools/`, `editors/` y `website/` identifica los componentes no empaquetados como crates.

El producto corre localmente como herramienta de desarrollo. Las APIs entre estos módulos son Rust tipado dentro del proceso. La comunicación distribuida, almacenamiento de negocio y telemetría remota no forman parte de esta arquitectura. La CLI exporta, compila y ejecuta la rebanada inicial de `.ox`; el objetivo es la interoperabilidad amplia con Rust, cuya cobertura se amplía y verifica progresivamente en el laboratorio de conformidad.

## Estado de los directorios

El LSP comparte análisis con el compilador y, bajo confianza explícita, ofrece consultas mediante `rust-analyzer` y comprobaciones Cargo; las limitaciones semánticas y de plataforma están en la [guía de VS Code y LSP](vscode.md). `website/` contiene el portal Astro Starlight; las páginas de referencia se generan desde Markdown canónico en `docs/`. `oxido formato` conserva comentarios e idempotencia probados en ambos modos; el Rust que exporta pasa por `rustfmt` 1.99.1. La matriz estructural cubre las producciones listadas en `testing.md`, pero no enumera exhaustivamente cada token o expresión que Syn delega a Rust; esa limitación se conserva explícita. `examples/` y `tests/` contienen los flujos reproducibles disponibles. La separación no obliga a crear un crate por cada carpeta interna.
