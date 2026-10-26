# Decisiones de arquitectura (ADR)

Los ADR registran decisiones con consecuencias duraderas para el compilador, sus herramientas o sus contratos. No existe la regla de que cada fase deba tener un ADR: las fases que solo completan trabajo ya decidido pueden referenciar el ADR propietario en lugar de duplicarlo.

## Decisiones

- [0001 — Parser estructural con fragmentos Rust preservados](0001-parser-hibrido.md): frontera entre el parser de Óxido y la sintaxis Rust.
- [0002 — Generación trazable y adaptador Cargo](0002-generacion-y-proceso-cargo.md): generación tokenizada, mapas de origen y procesos Cargo.
- [0003 — AST y generación tokenizada para items indentados](0003-items-indentados-iniciales.md): estructura inicial de items indentados.
- [0004 — Items y coincidencia indentados preservan Rust](0004-estructura-indentada-rust.md): bloques indentados y preservación estructural.
- [0005 — Dependencias Cargo versionadas](0005-dependencias-cargo-versionadas.md): validación y resolución de requisitos de versión.
- [0006 — Descargar y confiar dependencias Cargo](0006-descarga-y-confianza-de-dependencias.md): fetch, lockfile y activación de código de dependencias.
- [0007 — Contrato de interoperabilidad con Rust](0007-contrato-interoperabilidad-rust.md): objetivo semántico y límites de compatibilidad (Fase 8).
- [0008 — Contrato de formato de Óxido y Rust generado](0008-contrato-de-formato.md): formato propio de `.ox` y conformidad del Rust exportado (Fase 9).
- [0009 — Evidencia reproducible y alcance de benchmarks](0009-politica-de-evidencia-y-benchmarks.md): evidencia de pruebas, fuzz y mediciones (Fase 10).
- [0010 — Frontera del LSP y confianza del editor](0010-frontera-lsp-y-confianza-editor.md): análisis estático, Workspace Trust y ejecución de herramientas editoriales (Fase 11).
