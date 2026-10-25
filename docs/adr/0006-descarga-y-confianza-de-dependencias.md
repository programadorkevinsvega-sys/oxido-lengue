# ADR 0006: Descargar y confiar dependencias Cargo

- **Estado:** Aceptada
- **Fase:** 8, interoperabilidad Rust

## Contexto

Resolver y descargar dependencias no debe ejecutar sus `build.rs` ni macros procedurales. Compilar sí puede ejecutarlos, por lo que el consentimiento debe ser visible y separado de la descarga.

## Decisión

- `oxido descargar-dependencias` primero imprime los paquetes fijados, el cambio de paquetes en `Cargo.lock` y las crates de crates.io que Cargo puede necesitar descargar.
- Sin `--confirmar`, solo genera el plan en un workspace temporal; no descarga crates ni escribe el lockfile del proyecto. Con `--confirmar`, ejecuta `cargo fetch --locked` y persiste el lockfile solo si Cargo termina correctamente. `--actualizar` prepara una nueva resolución que requiere también confirmación.
- Crates de crates.io son la única fuente permitida por defecto. Las dependencias `path` requieren `--permitir-rutas-locales`; git y URL no están soportadas. `oxido.toml` no puede activar esta política.
- `compilar` y `ejecutar` con dependencias requieren el flag de CLI `--confiar-dependencias`. Este flag autoriza a Cargo a ejecutar código de build y macros durante esa acción; no afirma que el código sea seguro ni lo sandboxa.
- `compilar`/`ejecutar` siguen consumiendo `Cargo.lock` con `--locked --offline`. Si hay cualquier dependencia y falta el lockfile se devuelve `OX7001`; no se crea el lockfile persistente.

## Verificación

Las pruebas CLI comprueban que la vista previa no persiste el lockfile, que fetch no ejecuta un fixture `build.rs` ni una macro procedural, que ambos se ejecutan solo después de la compilación confiada, y que cambios en el vendor o checksum del lockfile hacen fallar Cargo. También comprueban el rechazo por defecto de fuentes locales y la exportación `.rs` desde distintos directorios.

## Consecuencias y límites

El flag de confianza es por invocación, no un sandbox. El operador debe revisar el proyecto y sus dependencias. Las fuentes git y URL siguen sin soporte. El lockfile por sí solo no valida que una dependencia sea benigna.
