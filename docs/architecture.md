# Arquitectura de Óxido

El contrato normativo está en [ARCHITECTURE_RULES.md](../ARCHITECTURE_RULES.md). Este documento resume los crates que ya existen y los límites que siguen preparados para fases posteriores.

La decisión de limitar el parser propio a estructura Óxido y preservar fragmentos Rust está registrada en [ADR-0001](adr/0001-parser-hibrido.md). El objetivo normativo de interoperabilidad amplia con Rust, sin modificar bibliotecas externas, está en [ADR-0007](adr/0007-contrato-interoperabilidad-rust.md). La cobertura incompleta es una brecha de implementación/pruebas, no un límite permanente del lenguaje.

## Componentes

| Componente | Responsabilidad | Tipo de módulo |
|---|---|---|
| `oxido-language` | Fuente/spans UTF-8, diagnósticos, lexer, AST estructural y parser de `.ox` | Núcleo/dominio |
| `oxido-compiler` | Casos de uso de parseo, generación, mapa, integración Cargo e importación reversible | Aplicación |
| `oxido-rust-backend` | Emitir Rust desde el AST con quote/proc_macro2; validar y formatear con Syn/prettyplease | Adapter de salida |
| `oxido-cli` | `analizar-rust`, `nuevo`, `formato`, `exportar`, `importar`, `importar-proyecto`, `compilar`, `probar` y `ejecutar` | Adapter de entrada |
| `oxido-cargo` | Invocar Cargo por argv, limitar tiempo y leer mensajes JSON | Adapter de proceso |
| `oxido-lsp` | Transporte stdio y diagnósticos léxicos iniciales | Prototipo; semantic tokens heurísticos no fiables; no usa `rust-analyzer`, no compila y no implementa Workspace Trust |
| `editors/vscode-oxido` | Documentación/preparación para cliente VS Code | No hay extensión funcional |
| `website` | Portal estático Markdown con documentación por audiencia y referencia | Frontend documental; Astro Starlight |

El producto corre localmente como herramienta de desarrollo. Las APIs entre estos módulos son Rust tipado dentro del proceso. La comunicación distribuida, almacenamiento de negocio y telemetría remota no forman parte de esta arquitectura. La CLI exporta, compila y ejecuta la rebanada inicial de `.ox`; el objetivo es la interoperabilidad amplia con Rust, cuya cobertura se amplía y verifica progresivamente en el laboratorio de conformidad.

## Estado de los directorios

Fases 2–8 materializaron el workspace, lexer/parser, generación, CLI e interoperabilidad inicial. `website/` contiene el portal Astro Starlight; las páginas de referencia se generan desde Markdown canónico en `docs/`. `oxido-lsp` es un prototipo de transporte y análisis léxico; sus semantic tokens son heurísticos y no fiables, y no se conecta con `rust-analyzer`. No existe una extensión VS Code funcional ni Workspace Trust integrado. `oxido formato` conserva comentarios e idempotencia probados en modo llaves; en modo indentado normaliza sangría y espaciado del corpus probado, y la paridad completa con `rustfmt` sigue abierta en Fase 9. `examples/` y `tests/` contienen los flujos reproducibles disponibles. La separación no obliga a crear un crate por cada carpeta interna.
