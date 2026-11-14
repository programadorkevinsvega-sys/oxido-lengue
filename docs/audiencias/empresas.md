# Óxido para empresas

## Evaluación honesta del estado

Óxido aún no es un compilador listo para producción. Para evaluar la CLI en Debian o Ubuntu se puede instalar el `.deb` de la [Release v0.1.1](../instalacion.md) y usar `oxido nuevo`, `oxido verificar`, `oxido compilar` y `oxido ejecutar`; no es necesario clonar ni compilar el repositorio. Hay ejemplos experimentales que exportan, compilan y ejecutan: Hola, mundo; items indentados; módulos; y casos de interoperabilidad Rust con métodos, referencias, `Result`/`Option`, colecciones y macros concretas. Las dependencias Cargo de registry y locales se prueban con lockfile persistente y compilación offline. La matriz delimita exactamente el corpus; la CLI no sandboxa código de dependencias. El LSP y la extensión de VS Code cubren diagnósticos, semantic tokens, Workspace Trust `limited` y, en workspace confiable, navegación y renombrado con `rust-analyzer`. Conformidad amplia, sandbox general y artefactos de distribución firmados no se presentan como capacidades disponibles.

No debe adoptarse para cargas de producción hasta que esos criterios estén implementados, auditados y publicados con evidencia. Esta página no constituye una certificación ni una garantía de seguridad.

## Controles existentes

- El compilador propio prohíbe `unsafe` en sus crates.
- El lexer limita fuente a 4 MiB, grupos/comentarios anidados a 256 y tokens a 1.000.000.
- Rechaza controles bidi y avisa sobre mezcla de alfabetos latino, griego y cirílico en identificadores.
- `oxido.toml` se parsea como TOML de datos, con campos cerrados; no ejecuta comandos.
- El workspace Rust del propio repositorio tiene lockfile; la CLI usa `--locked` en su workspace temporal.

Óxido admite dependencias de crates.io; las dependencias locales `path` exigen `--permitir-rutas-locales`. La descarga muestra antes el plan y requiere `--confirmar`; solo usa `cargo fetch`, nunca compila. `check`, `build` y `run` consumen el lockfile con `--locked --offline`; con dependencias requieren `--confiar-dependencias`. Para un proyecto `.ox`, `build --release` genera un proyecto Cargo en `target/oxido/` y los binarios en `target/oxido/target/release`; mantiene allí tanto el Rust generado como las fuentes `.ox`, incluidos los módulos `mod.ox`. Cargo puede ejecutar `build.rs` o macros procedurales durante compilación; el flag registra confianza del operador, no aísla código ni bloquea su acceso a red. Workspace Trust está implementado para el editor; el sandbox general y la seguridad de lanzamiento siguen pendientes. Hay fuzzing inicial para lexer, parser, backend, configuración y transporte LSP, además de pruebas diferenciales y generación local de informes; esto no es una auditoría completa del compilador. El detalle y la lista de evaluación están en [Preparación empresarial](../enterprise-readiness.md).

## Casos de evaluación

1. Revisar la [matriz de compatibilidad](../compatibility.md) para identificar la sintaxis soportada.
2. Probar el lexer con un corpus interno sin datos sensibles.
3. Reproducir `exportar`, `compilar` y `ejecutar` con los [ejemplos del repositorio](https://github.com/programadorkevinsvega-sys/oxido-lengue/tree/main/examples).
4. Evaluar amenazas y dependencias antes de confiar el proyecto a CI o editores automáticos.

Consulta la [matriz de compatibilidad](../compatibility.md), los [diagnósticos](../diagnostics.md) y la [preparación empresarial](../enterprise-readiness.md).
