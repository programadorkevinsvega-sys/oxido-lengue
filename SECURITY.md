# Seguridad

Los archivos `.ox`, `oxido.toml`, las dependencias, las macros y las solicitudes LSP se tratan como entradas no confiables. No ejecutes proyectos no confiables desde el editor sin Workspace Trust.

Las compilaciones pueden ejecutar `build.rs` y macros procedurales de dependencias. Óxido exige una autorización explícita para compilar dependencias y usa `--locked --offline` en las rutas de compilación documentadas. Reporta vulnerabilidades sin publicar detalles explotables en un issue público: contacta a los mantenedores del repositorio.

La política completa está en `plan`, `AGENTS.md` y `docs/fase-8-integracion-rust.md`.
