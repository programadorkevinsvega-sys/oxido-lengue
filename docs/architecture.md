# Arquitectura de Óxido

El contrato normativo está en [ARCHITECTURE_RULES.md](../ARCHITECTURE_RULES.md). Este documento resume los crates que ya existen y los límites que siguen preparados para fases posteriores.

## Componentes

| Componente | Responsabilidad | Tipo de módulo |
|---|---|---|
| `oxido-language` | Fuente/spans UTF-8, diagnósticos, lexer, AST estructural y parser de `.ox` | Núcleo/dominio |
| `oxido-compiler` | Casos de uso de análisis `.ox` y validación de fragmentos Rust/configuración | Aplicación |
| `oxido-rust-backend` | Parsear y formatear AST Rust con Syn/prettyplease | Adapter de salida |
| `oxido-cli` | Comando local `analizar-rust` y composición inicial | Adapter de entrada |
| `oxido-cargo` | Invocar Cargo/rustc y leer diagnósticos JSON | Previsto para fase posterior |
| `oxido-lsp` | Protocolo de editor que reutiliza compiler | Previsto para fase posterior |
| `editors/vscode-oxido` | Integración y presentación en VS Code | Cliente separado |
| `website` | Portal estático Markdown con documentación por audiencia y referencia | Frontend documental; Astro Starlight |

El producto corre localmente como herramienta de desarrollo. Las APIs entre estos módulos son Rust tipado dentro del proceso. La comunicación distribuida, almacenamiento de negocio y telemetría remota no forman parte de esta arquitectura. La biblioteca ya tokeniza y parsea el subconjunto inicial de `.ox`; el comando de análisis Rust sigue siendo una comprobación separada. Aún no existe traducción `.ox → .rs` ni invocación de Cargo.

## Estado de los directorios

Fase 2 materializó cuatro paquetes: `oxido-language`, `oxido-rust-backend`, `oxido-compiler` y `oxido-cli`. Fase 3 añadió el lexer y Fase 4 el parser/AST inicial. `website/` contiene el frontend documental en Astro Starlight; sus páginas de referencia se generan desde los Markdown canónicos en `docs/` para evitar mantener dos copias editables. Las carpetas `oxido-cargo`, `oxido-lsp`, `editors/`, `tests/fixtures/` y `examples/` siguen como scaffolds para sus fases. La separación no obliga a crear un crate por cada carpeta interna.
