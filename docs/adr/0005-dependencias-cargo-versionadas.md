# ADR 0005: Dependencias Cargo versionadas

- **Estado:** Aceptada
- **Fase:** 8, integración con Cargo

## Contexto

Los proyectos Óxido necesitan declarar crates por ruta local o por requisito de versión de crates.io. Las versiones deben seguir exactamente la gramática SemVer que Cargo acepta; reimplementar esa gramática en el compilador introduciría reglas incompletas y divergencias.

## Decisión

- Usar `semver::VersionReq` para validar los requisitos de versión al leer `oxido.toml`.
- Aceptar exactamente una fuente: `path` relativa al proyecto o `version`; las rutas locales siguen sujetas a canonicalización dentro del proyecto.
- No aceptar fuentes git, URL, comandos, parches ni opciones libres en esta fase.
- Permitir `features` y `default-features` como datos declarativos con límites de tamaño; Cargo interpreta sus nombres.
- Versionar `Cargo.lock`; compilar y ejecutar deben consumirlo con `--locked --offline`. Si hay cualquier dependencia y falta el lockfile, fallan con `OX7001` y una instrucción clara sin crear el lockfile persistente de forma implícita.
- La resolución/descarga requiere el comando explícito `oxido descargar-dependencias`. Por defecto solo se admiten crates de crates.io; una fuente local `path` requiere `--permitir-rutas-locales`, y git/URL siguen rechazadas.
- Descargar presenta primero un plan de paquetes y cambios al lockfile. Sin `--confirmar` no descarga paquetes ni escribe `Cargo.lock`; `--actualizar` muestra la nueva resolución y solo la aplica con `--confirmar`.
- `descargar-dependencias` usa `cargo generate-lockfile` y `cargo fetch --locked`; no ejecuta `cargo check`, `build` ni `run`. La compilación y ejecución posterior requieren `--confiar-dependencias`, porque Cargo puede ejecutar `build.rs` y macros procedurales.
- La CLI no crea automáticamente `Cargo.lock` durante `compilar`/`ejecutar`; si falta, se informa el fallo y se indica descargar/resolver dependencias explícitamente.

## Consecuencias

`semver` añade una sola dependencia directa pequeña para compartir la sintaxis de versión de Cargo. No se versionan rutas absolutas ni la caché local. El vendor fixture incluye `itoa` y `futures-executor`, ambos con licencias MIT/Apache-2.0 y checksums verificados; la prueba usa `CARGO_HOME` aislado para no depender de la caché del mantenedor ni de la red.

El diff del `Cargo.lock` del compilador añade solamente `semver 1.0.28` y su entrada directa; no actualiza otras versiones. `cargo-deny` permite MIT, Apache-2.0 y Unicode-3.0 para dependencias externas. Los crates workspace se marcan `publish = false` porque no tienen licencia de publicación elegida. La licencia del repositorio/producto no se decide en este ADR.

## Riesgo pendiente

`--offline` limita las descargas de Cargo, pero no sandboxa `build.rs` ni macros procedurales. El flag de confianza es una decisión explícita del operador, no un sandbox. Workspace Trust y aislamiento de procesos siguen siendo requisitos para LSP/playground.
