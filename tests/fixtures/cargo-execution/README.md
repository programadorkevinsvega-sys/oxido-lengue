# Fixtures de ejecución de código Cargo

Estos dos crates son deliberadamente mínimos y locales. `build-script` escribe un marcador y emite un warning; `proc-macro` escribe un marcador cuando se invoca su atributo identity. Las pruebas demuestran que `oxido descargar-dependencias` no ejecuta ninguno y que `compilar` solo los activa con `--confiar-dependencias`.

Los marcadores se pasan mediante variables de entorno únicamente desde la prueba, para observar la ejecución sin depender del orden o del contenido de logs de Cargo.
