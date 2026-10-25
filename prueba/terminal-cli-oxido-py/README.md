# Terminal CLI en Óxido

Esta es la conversión del proyecto de `terminal-cli-rust` a archivos `.ox` con bloques indentados y palabras clave en español. Se conservan las carpetas, los tipos, todas las funciones y los tres tests del parser. Las operaciones de la terminal siguen siendo `help`, `pwd`, `ls`, `cd`, `cat`, `echo`, `clear`, `exit` y `quit`.

## Compilar y ejecutar el binario interactivo

Desde la raíz del repositorio, exporta el árbol y compila el Rust nativo. Los archivos generados quedan bajo `target/`, sin añadir otro proyecto fuente ni dependencias:

```sh
mkdir -p target/terminal-cli-oxido-py/src
cargo run --locked -p oxido-cli -- exportar prueba/terminal-cli-oxido-py/src/main.ox --salida target/terminal-cli-oxido-py/src/main.rs
rustc target/terminal-cli-oxido-py/src/main.rs -o target/terminal-cli-oxido-py/terminal-cli
```

Ejecuta el binario desde la carpeta del proyecto para que `pwd`, `ls` y las rutas relativas empiecen allí:

```sh
cd prueba/terminal-cli-oxido-py
../../target/terminal-cli-oxido-py/terminal-cli
```

`oxido compilar` comprueba el proyecto, pero su workspace temporal no es el binario interactivo persistente. Por eso la ejecución manual usa el Rust exportado y compilado.

## Tests

Desde la raíz, exporta el proyecto y ejecuta los tests originales del parser con `rustc --test`:

```sh
rustc --test target/terminal-cli-oxido-py/src/main.rs -o target/terminal-cli-oxido-py/tests
target/terminal-cli-oxido-py/tests
```
