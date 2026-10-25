# Óxido para empresas

## Evaluación honesta del estado

Óxido aún no es un compilador listo para producción. Hay ejemplos experimentales que exportan, compilan y ejecutan: Hola, mundo; items `struct`/`enum` indentados; e interoperabilidad Rust con métodos, módulos, referencias, `Result`/`Option` y colecciones. La matriz delimita lo que esos casos no cubren; LSP integrado, dependencias de usuario, seguridad de lanzamiento y artefactos de distribución siguen pendientes.

No debe adoptarse para cargas de producción hasta que esos criterios estén implementados, auditados y publicados con evidencia. Esta página no constituye una certificación ni una garantía de seguridad.

## Controles existentes

- El compilador propio prohíbe `unsafe` en sus crates.
- El lexer limita fuente a 4 MiB, grupos/comentarios anidados a 256 y tokens a 1.000.000.
- Rechaza controles bidi y avisa sobre mezcla de alfabetos latino, griego y cirílico en identificadores.
- `oxido.toml` se parsea como TOML de datos, con campos cerrados; no ejecuta comandos.
- El workspace tiene lockfile y las verificaciones locales usan `--locked`.

Estos controles no sustituyen revisión de dependencias, sandbox de compilación, fuzzing, auditoría de rutas ni política de confianza del editor; varias de esas tareas siguen pendientes.

## Casos de evaluación

1. Revisar matriz de compatibilidad para identificar sintaxis soportada.
2. Probar el lexer con corpus interno sin datos sensibles.
3. Reproducir `exportar`, `compilar` y `ejecutar` con los ejemplos de `examples/README.md`.
4. Evaluar amenazas y dependencias antes de confiar el proyecto a CI o editores automáticos.

Consulta la [matriz de compatibilidad](/referencia/compatibility/), [diagnósticos](/referencia/diagnostics/) y [roadmap](/referencia/roadmap/).
