# Arquitectura de Óxido

El contrato normativo está en [ARCHITECTURE_RULES.md](../ARCHITECTURE_RULES.md). Este documento resume los crates que ya existen y los límites que siguen preparados para fases posteriores.

La decisión de limitar el parser propio a estructura Óxido y preservar fragmentos Rust está registrada en [ADR-0001](adr/0001-parser-hibrido.md). El objetivo normativo de interoperabilidad amplia con Rust, sin modificar bibliotecas externas, está en [ADR-0007](adr/0007-contrato-interoperabilidad-rust.md). La cobertura incompleta es una brecha de implementación/pruebas, no un límite permanente del lenguaje.

## Componentes

| Componente | Responsabilidad | Tipo de módulo |
|---|---|---|
| `oxido-language` | Fuente/spans UTF-8, diagnósticos, lexer, AST estructural y parser de `.ox` | Núcleo/dominio |
| `oxido-compiler` | Casos de uso de parseo, generación, mapa, integración Cargo e importación reversible | Aplicación |
| `oxido-rust-backend` | Emitir Rust con quote/proc_macro2; validar con Syn, formato intermedio prettyplease y formato final rustfmt 1.99.1 | Adapter de salida |
| `oxido-cli` | `analizar-rust`, `nuevo`, `formato`, `exportar`, `importar`, `importar-proyecto`, `compilar`, `probar` y `ejecutar` | Adapter de entrada |
| `oxido-cargo` | Invocar Cargo por argv, limitar tiempo y leer mensajes JSON | Adapter de proceso |
| `oxido-lsp` | Transporte stdio, sincronización de documentos, frontend compartido, diagnósticos y semantic tokens | Parcial: no invoca Cargo ni `rust-analyzer`; límites por subfase en [Fase 11](fase-11-vscode-lsp.md) |
| `editors/vscode-oxido` | Cliente VS Code para `.ox`, configuración y comandos editoriales iniciales | Extensión mínima en desarrollo; declara Workspace Trust `limited`; host test cubre activación y diagnóstico básico en modo confiable/restringido |
| `website` | Portal estático Markdown con documentación por audiencia y referencia | Frontend documental; Astro Starlight |

El producto corre localmente como herramienta de desarrollo. Las APIs entre estos módulos son Rust tipado dentro del proceso. La comunicación distribuida, almacenamiento de negocio y telemetría remota no forman parte de esta arquitectura. La CLI exporta, compila y ejecuta la rebanada inicial de `.ox`; el objetivo es la interoperabilidad amplia con Rust, cuya cobertura se amplía y verifica progresivamente en el laboratorio de conformidad.

## Estado de los directorios

Fases 2–10 materializaron el workspace, lexer/parser, generación, CLI, interoperabilidad inicial y laboratorio. La Fase 11 está en curso: existe extensión VS Code mínima y el LSP consume lexer/parser reales; un host test verifica diagnóstico básico en dos estados de confianza, pero aún faltan las capacidades marcadas en su [informe](fase-11-vscode-lsp.md), en particular `rust-analyzer` y diagnósticos Cargo. El contrato de confianza está en [ADR 0010](adr/0010-frontera-lsp-y-confianza-editor.md). `website/` contiene el portal Astro Starlight; las páginas de referencia se generan desde Markdown canónico en `docs/`. `oxido formato` conserva comentarios e idempotencia probados en ambos modos; el Rust que exporta pasa por `rustfmt` 1.99.1. La matriz estructural de Fase 9 cubre las producciones listadas en `testing.md`, pero no enumera exhaustivamente cada token/expresión que Syn delega a Rust; esa limitación se conserva explícita. `examples/` y `tests/` contienen los flujos reproducibles disponibles. La separación no obliga a crear un crate por cada carpeta interna.
