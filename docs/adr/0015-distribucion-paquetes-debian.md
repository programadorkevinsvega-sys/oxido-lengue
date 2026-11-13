# ADR 0015: Distribución de paquetes Debian

- **Estado:** Aceptado.
- **Fecha:** 2026-11-13.

## Contexto

Óxido ya tiene un constructor de `.deb` para el host nativo y pruebas de instalación, actualización y eliminación. La distribución inicial debe entregar paquetes Linux sin reemplazar Rust del sistema o del usuario, y sin ejecutar descargas desde scripts mantenedores de `dpkg`.

## Decisión

- CI construye el paquete en runners nativos amd64 y arm64; no se usa compilación cruzada para formar los paquetes.
- Pull requests y ejecuciones manuales construyen y validan artefactos, pero no crean ni publican Releases.
- Un push de etiqueta `v` cuya versión coincida exactamente con `oxido-cli` crea o actualiza una GitHub Release con ambos paquetes y `SHA256SUMS`.
- La publicación usa `GITHUB_TOKEN` con `contents: write` únicamente en el job que adjunta los artefactos. Los demás jobs conservan solo lectura.
- La instalación de Rust permanece explícita mediante `oxido-instalar-toolchain`; instalar el `.deb` no descarga ni ejecuta Rustup.
- Los checksums detectan alteraciones accidentales, pero no sustituyen una firma criptográfica. Los paquetes no se declaran firmados.

## Consecuencias

La descarga e instalación son directas desde GitHub Releases y no requieren mantener un repositorio APT. Una versión no estará disponible hasta que el autor cree y empuje su etiqueta. La firma de paquetes, el repositorio APT y los binarios de CLI para otros sistemas quedan fuera de esta decisión.
