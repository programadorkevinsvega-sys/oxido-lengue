# Ejemplos

`hola_mundo.ox` es el ejemplo mínimo ejecutable y forma parte de la prueba de extremo a extremo de la CLI. Otros ejemplos se añadirán con pruebas antes de describirse como ejecutables.

Desde la raíz del repositorio:

```sh
cargo run --locked -p oxido-cli -- exportar examples/hola_mundo.ox --salida target/hola.rs
cargo run --locked -p oxido-cli -- compilar examples/hola_mundo.ox
cargo run --locked -p oxido-cli -- ejecutar examples/hola_mundo.ox
```
