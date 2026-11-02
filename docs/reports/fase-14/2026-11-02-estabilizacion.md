# Fase 14: preparación de estabilización

**Fecha:** 2026-11-02

**Estado:** preparación, no lanzamiento. Este informe reúne la evidencia disponible para un candidato de versión y enumera los bloqueos. **No se ha publicado, firmado ni desplegado nada.**

## Alcance congelado propuesto

- Versión de todos los crates: `0.1.0`.
- Compatibilidad: la [matriz](../../compatibility.md) vigente, sin ampliarla en esta revisión.
- Plataformas de la CLI: Linux, Windows y macOS según CI; el sandbox del playground solo Linux ([ADR 0013](../../adr/0013-cierre-12-11-alcance-linux.md)).
- Editor: VS Code con `oxido-lsp`, VSIX sin firmar y sin marketplace.
- Licencia: MIT o Apache-2.0.
- Sin backend, telemetría ni servicios externos nuevos.

Este alcance es una propuesta de trabajo. No equivale a aceptación formal de lanzamiento.

## Evidencia ejecutada

| Comprobación | Resultado |
|---|---|
| `cargo fmt --all -- --check` | Sin cambios pendientes |
| `cargo clippy --workspace --all-targets --locked -- -D warnings` | Sin advertencias |
| `cargo test --workspace --locked` | 222 pruebas, 0 fallos |
| `cargo audit --quiet` | Sin vulnerabilidades conocidas con la base actual; código de salida 0 |
| `cargo deny check` | `advisories ok, bans ok, licenses ok, sources ok` |
| `tools/verify-reproducible-build.sh` | `oxido-cli/oxido` release reproducible; SHA-256 `2718f8eee1c3a1a5ca08b6b603db53f73121ff6d2f7bec0885d29d40e21afb06` |
| `tools/generate-sbom.sh` | 8 SBOM CycloneDX sin rutas absolutas |
| `cargo install --path crates/oxido-cli --locked` | Instala el binario `oxido` en un prefijo temporal |
| `cargo run --locked -p oxido-cli -- ejecutar prueba/interoperabilidad_std/main.ox` | 4 pruebas de biblioteca estándar completadas |
| CI `reference-regressions` (run `41543758070`) | Interoperabilidad `std`, CLI de terminal, CRUD concurrente, persistencia SQLite y exportación `.ox` → `.rs` en verde |
| `npm --prefix website run build` | 58 páginas, enlaces internos sin destinos rotos |

Los resultados de CI se refieren al commit de `main` posterior a la fusión de la documentación de Fase 13. Las comprobaciones locales se ejecutaron en Linux con Rust 1.99.1.

## Revisión de subfases

1. **Congelar alcance y matriz.** Propuesta registrada arriba; falta aceptación formal.
2. **Suite, auditorías, fuzzing y proyectos de referencia.** Suite y auditorías ejecutadas; el fuzzing smoke y las regresiones de referencia pasan en CI. La campaña semanal de fuzz no se relanzó en esta revisión.
3. **Instalación limpia y toolchain.** `cargo install` verificado; el toolchain de CI está fijado en Rust 1.99.1.
4. **Seguridad, dependencias, licencias, SBOM, firma y reproducibilidad.** Auditoría, `cargo-deny`, SBOM y reproducibilidad verificados. Firma y notarización no existen.
5. **Regresiones bloqueantes.** No se observaron fallos bloqueantes en la última ejecución. La prueba de `reference-regressions` tuvo un fallo intermitente por una respuesta HTTP incompleta; se corrigió el sondeo y la ejecución posterior pasó.
6. **Changelog, migración, limitaciones y soporte.** Changelog y limitaciones están documentados. No hay notas de migración porque no existe una versión anterior publicada ni soporte comprometido.
7. **Artefactos y procedimiento de publicación.** Reproducibilidad y SBOM existen. Faltan firma, checksums publicados y un procedimiento aprobado.
8. **Publicación con autorización.** Bloqueada: no hay autorización ni credenciales.

## Bloqueos para declarar un lanzamiento

1. **Autorización.** Publicar, firmar o desplegar requiere una decisión explícita del titular. No se hizo ninguna de esas operaciones.
2. **Firma y notarización.** Sin credenciales ni proceso de verificación de actualizaciones.
3. **Distribución.** No hay binarios publicados, instaladores, marketplace ni canal de soporte.
4. **Playground.** Solo Linux, sin despliegue público ni cuotas de producción.
5. **Auditoría externa.** No hay revisión de seguridad independiente del compilador ni de la cadena de suministro.
6. **Alcance del lenguaje.** La matriz sigue siendo parcial; un programa que compila no demuestra compatibilidad general.
7. **Validación educativa.** Los pilotos de Fase 13 no se han ejecutado.

## Conclusión

El repositorio tiene un candidato técnico reproducible y con evidencia de pruebas, auditoría, SBOM y regresiones de referencia. **No es un lanzamiento:** faltan autorización, firma, distribución, auditoría externa y validación de usuarios. Cualquier excepción que se acepte debe tener responsable, justificación y estado explícito, según el criterio de salida de la fase.
