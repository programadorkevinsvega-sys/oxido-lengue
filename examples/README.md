# Ejemplos

`hola_mundo.ox` es el ejemplo mínimo ejecutable y forma parte de la prueba de extremo a extremo de la CLI. Otros ejemplos se añadirán con pruebas antes de describirse como ejecutables.

Desde la raíz del repositorio:

```sh
cargo run --locked -p oxido-cli -- exportar examples/hola_mundo.ox --salida target/hola.rs
cargo run --locked -p oxido-cli -- compilar examples/hola_mundo.ox
cargo run --locked -p oxido-cli -- ejecutar examples/hola_mundo.ox
```

`estructuras.ox` demuestra el subconjunto indentado de `struct`, `enum`, `use`, arrays, mutabilidad y colecciones. `interoperabilidad.ox` demuestra interoperabilidad Rust en modo llaves: módulos, métodos, tuplas, referencias, `Result`, `Option`, `?` y `VecDeque`. `prelude_espanol.ox` demuestra `dejar mutable`, `principal`, `Vector<Cadena>`, `&texto`, métodos y aliases de macros; los argumentos de macros conservan Rust. Los ejemplos ejecutables tienen pruebas que compilan y corren mediante Cargo.

`estructura_indentada.ox` combina estructuras, `implementar`, `rasgo`, módulos anidados y `coincidir` con inicializador y brazos con bloque indentado. Cubre la primera entrega de integración con Rust, no el cierre de toda la interoperabilidad.

`tutorial/01_saludo.ox`, `tutorial/02_condiciones.ox` y `tutorial/03_punto.ox` son las tres lecciones del [tutorial progresivo](../docs/tutorial.md). También tienen prueba automática de compilación y salida.

`ejercicios/` contiene archivos de inicio y `ejercicios/soluciones/` las soluciones de referencia del [libro de ejercicios](../docs/ejercicios.md). Las soluciones se compilan y ejecutan en las pruebas; los archivos de inicio son incompletos a propósito.

```sh
cargo run --locked -p oxido-cli -- ejecutar examples/estructuras.ox
cargo run --locked -p oxido-cli -- ejecutar examples/interoperabilidad.ox
cargo run --locked -p oxido-cli -- ejecutar examples/prelude_espanol.ox
cargo run --locked -p oxido-cli -- ejecutar examples/estructura_indentada.ox
```
