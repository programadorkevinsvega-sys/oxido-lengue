# Arquitectura de Óxido

El contrato normativo está en [ARCHITECTURE_RULES.md](../ARCHITECTURE_RULES.md). Este documento resume los crates que ya existen y los límites que siguen preparados para fases posteriores.

La decisión de limitar el parser propio a estructura Óxido y preservar fragmentos Rust está registrada en [ADR-0001](adr/0001-parser-hibrido.md). El objetivo normativo de interoperabilidad amplia con Rust, sin modificar bibliotecas externas, está en [ADR-0007](adr/0007-contrato-interoperabilidad-rust.md). La cobertura incompleta es una brecha de implementación/pruebas, no un límite permanente del lenguaje.

## Componentes

| Componente | Responsabilidad | Tipo de módulo |
|---|---|---|
| `oxido-language` | Fuente/spans UTF-8, diagnósticos, lexer, AST estructural y parser de `.ox` | Núcleo/dominio |
| `oxido-compiler` | Casos de uso de parseo, generación, mapa, integración Cargo e importación reversible | Aplicación |
| `oxido-rust-backend` | Emitir Rust desde el AST con quote/proc_macro2; validar y formatear con Syn/prettyplease | Adapter de salida |
| `oxido-cli` | `analizar-rust`, `exportar`, `importar`, `compilar` y `ejecutar` | Adapter de entrada |
| `oxido-cargo` | Invocar Cargo por argv, limitar tiempo y leer mensajes JSON | Adapter de proceso |
| `oxido-lsp` | Protocolo de editor que reutiliza compiler | Previsto para fase posterior |
| `editors/vscode-oxido` | Integración y presentación en VS Code | Cliente separado |
| `website` | Portal estático Markdown con documentación por audiencia y referencia | Frontend documental; Astro Starlight |

El producto corre localmente como herramienta de desarrollo. Las APIs entre estos módulos son Rust tipado dentro del proceso. La comunicación distribuida, almacenamiento de negocio y telemetría remota no forman parte de esta arquitectura. La CLI exporta, compila y ejecuta la rebanada inicial de `.ox`; el objetivo es la interoperabilidad amplia con Rust, cuya cobertura se amplía y verifica progresivamente en el laboratorio de conformidad.

## Estado de los directorios

Fase 2 materializó cuatro paquetes base y Fase 5 incorporó `oxido-cargo` al workspace como adaptador funcional. Fase 3 añadió el lexer y Fase 4 el parser/AST inicial. `website/` contiene el frontend documental en Astro Starlight; sus páginas de referencia se generan desde los Markdown canónicos en `docs/` para evitar mantener dos copias editables. `oxido-lsp` y `editors/` siguen preparados para sus fases; `examples/hola_mundo.ox` y `tests/` contienen el primer flujo end-to-end. La separación no obliga a crear un crate por cada carpeta interna.
