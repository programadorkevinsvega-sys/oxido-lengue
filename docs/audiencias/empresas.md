# Óxido para empresas

## Evaluación honesta del estado

Óxido aún no es un compilador listo para producción. La base del workspace, lexer y parser/AST inicial existen; la traducción `.ox → .rs`, la compilación Cargo desde Óxido, el LSP integrado y los artefactos de lanzamiento están pendientes.

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
3. Mantener la compilación del código Rust generado bajo el control de Cargo/Rust.
4. Evaluar amenazas y dependencias antes de confiar el proyecto a CI o editores automáticos.

Consulta la [matriz de compatibilidad](/referencia/compatibility/), [diagnósticos](/referencia/diagnostics/) y [roadmap](/referencia/roadmap/).
