# Óxido para empresas

## Evaluación honesta del estado

Óxido aún no es un compilador listo para producción. Hay ejemplos experimentales que exportan, compilan y ejecutan: Hola, mundo; items `struct`/`enum` indentados; e interoperabilidad Rust con métodos, módulos, referencias, `Result`/`Option` y colecciones. También hay una prueba de dependencia Cargo local por ruta. La matriz delimita esos alcances; crates de registro, lockfile persistente, LSP integrado, seguridad de lanzamiento y artefactos de distribución siguen pendientes.

No debe adoptarse para cargas de producción hasta que esos criterios estén implementados, auditados y publicados con evidencia. Esta página no constituye una certificación ni una garantía de seguridad.

## Controles existentes

- El compilador propio prohíbe `unsafe` en sus crates.
- El lexer limita fuente a 4 MiB, grupos/comentarios anidados a 256 y tokens a 1.000.000.
- Rechaza controles bidi y avisa sobre mezcla de alfabetos latino, griego y cirílico en identificadores.
- `oxido.toml` se parsea como TOML de datos, con campos cerrados; no ejecuta comandos.
- El workspace Rust del propio repositorio tiene lockfile; la CLI usa `--locked` en su workspace temporal.

Óxido admite experimentalmente dependencias Cargo locales por ruta durante comandos explícitos de compilación/ejecución. Pueden ejecutar `build.rs` o macros procedurales. `--offline` evita que Cargo resuelva o descargue paquetes de red, pero no bloquea por sí solo el acceso a red del código de build. Las dependencias de registro, lockfile persistente, sandbox, fuzzing y política de confianza del editor siguen pendientes.

## Casos de evaluación

1. Revisar matriz de compatibilidad para identificar sintaxis soportada.
2. Probar el lexer con corpus interno sin datos sensibles.
3. Reproducir `exportar`, `compilar` y `ejecutar` con los ejemplos de `examples/README.md`.
4. Evaluar amenazas y dependencias antes de confiar el proyecto a CI o editores automáticos.

Consulta la [matriz de compatibilidad](/referencia/compatibility/), [diagnósticos](/referencia/diagnostics/) y [roadmap](/referencia/roadmap/).
