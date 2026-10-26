# Óxido para empresas

## Evaluación honesta del estado

Óxido aún no es un compilador listo para producción. Hay ejemplos experimentales que exportan, compilan y ejecutan: Hola, mundo; items indentados; módulos; y casos de interoperabilidad Rust con métodos, referencias, `Result`/`Option`, colecciones y macros concretas. Las dependencias Cargo de registry y locales se prueban con lockfile persistente y compilación offline. La matriz delimita exactamente el corpus; la CLI no sandboxa código de dependencias. El LSP es un prototipo sin integración con `rust-analyzer`, Workspace Trust ni extensión VS Code funcional. Conformidad amplia, seguridad de lanzamiento y artefactos de distribución no se presentan como capacidades disponibles.

No debe adoptarse para cargas de producción hasta que esos criterios estén implementados, auditados y publicados con evidencia. Esta página no constituye una certificación ni una garantía de seguridad.

## Controles existentes

- El compilador propio prohíbe `unsafe` en sus crates.
- El lexer limita fuente a 4 MiB, grupos/comentarios anidados a 256 y tokens a 1.000.000.
- Rechaza controles bidi y avisa sobre mezcla de alfabetos latino, griego y cirílico en identificadores.
- `oxido.toml` se parsea como TOML de datos, con campos cerrados; no ejecuta comandos.
- El workspace Rust del propio repositorio tiene lockfile; la CLI usa `--locked` en su workspace temporal.

Óxido admite dependencias de crates.io; las dependencias locales `path` exigen `--permitir-rutas-locales`. La descarga muestra antes el plan y requiere `--confirmar`; solo usa `cargo fetch`, nunca compila. `compilar`/`ejecutar` exigen `--locked --offline` y `--confiar-dependencias`. Cargo puede ejecutar `build.rs` o macros procedurales durante compilación; el flag registra confianza del operador, no aísla código ni bloquea su acceso a red. Workspace Trust, sandbox y seguridad de lanzamiento siguen pendientes. La Fase 10 incluye fuzzing inicial para lexer, parser, backend, configuración y transporte LSP, dos diferenciales de comportamiento y generación local inicial de informes; esto no es una auditoría completa del compilador.

## Casos de evaluación

1. Revisar matriz de compatibilidad para identificar sintaxis soportada.
2. Probar el lexer con corpus interno sin datos sensibles.
3. Reproducir `exportar`, `compilar` y `ejecutar` con los ejemplos de `examples/README.md`.
4. Evaluar amenazas y dependencias antes de confiar el proyecto a CI o editores automáticos.

Consulta la [matriz de compatibilidad](/referencia/compatibility/), [diagnósticos](/referencia/diagnostics/) y [roadmap](/referencia/roadmap/).
