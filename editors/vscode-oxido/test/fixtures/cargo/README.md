# Fixture Cargo del host de VS Code

Este proyecto declara una dependencia real de crates.io (`itoa` 1.0) y fija
`Cargo.lock`. La prueba host configura un `CARGO_HOME` temporal cuyo
`config.toml` reemplaza crates.io por el directorio versionado
`tests/fixtures/cargo-registry/vendor`; así Cargo no usa la caché global del
desarrollador ni la red.

`check_error.ox` importa `util_error.ox`, que contiene un error de tipos
deliberado. El comando `Óxido: Comprobar con Cargo` debe devolver el
diagnóstico de rustc en `util_error.ox`, no en el archivo activo. La prueba
también comprueba que `Cargo.lock` no cambia y que una ejecución sin confianza
no invoca ningún proceso Cargo.

Si el vendor versionado no está disponible, la prueba host debe informar que
el caso Cargo no se evaluó; no se considera aprobado por una caché implícita.
