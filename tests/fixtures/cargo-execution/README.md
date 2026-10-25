# Fixtures de ejecución de código Cargo

Estos dos crates son deliberadamente mínimos y locales. `build-script` escribe un marcador y emite un warning; `proc-macro` escribe un marcador al invocar el atributo `identity` y expone `token_text!`, que convierte en texto los tokens recibidos sin transformarlos. Las pruebas demuestran que `oxido descargar-dependencias` no ejecuta ninguno y que `ejecutar` solo los activa con `--confiar-dependencias`; también comprueban que `si tipo verdadero` llega intacto a la macro funcional procedural.

Los marcadores se pasan mediante variables de entorno únicamente desde la prueba, para observar la ejecución sin depender del orden o del contenido de logs de Cargo.
