# Ejemplos

`hola_mundo.ox` es el ejemplo mínimo ejecutable y forma parte de la prueba de extremo a extremo de la CLI. Otros ejemplos se añadirán con pruebas antes de describirse como ejecutables.

Desde la raíz del repositorio:

```sh
cargo run --locked -p oxido-cli -- exportar examples/hola_mundo.ox --salida target/hola.rs
cargo run --locked -p oxido-cli -- compilar examples/hola_mundo.ox
cargo run --locked -p oxido-cli -- ejecutar examples/hola_mundo.ox
```

`fase_7_estructuras.ox` demuestra el subconjunto indentado de `struct`, `enum`, `use`, arrays, mutabilidad y colecciones. `fase_7_interoperabilidad.ox` demuestra interoperabilidad Rust en modo llaves: módulos, métodos, tuplas, referencias, `Result`, `Option`, `?` y `VecDeque`. `fase_7_prelude_espanol.ox` demuestra `variable mutable`, `principal`, `Vector<Cadena>`, `&texto`, métodos y macros alias; los argumentos de macros conservan Rust. Los ejemplos ejecutables tienen pruebas que compilan y corren mediante Cargo.

```sh
cargo run --locked -p oxido-cli -- ejecutar examples/fase_7_estructuras.ox
cargo run --locked -p oxido-cli -- ejecutar examples/fase_7_interoperabilidad.ox
cargo run --locked -p oxido-cli -- ejecutar examples/fase_7_prelude_espanol.ox
```
