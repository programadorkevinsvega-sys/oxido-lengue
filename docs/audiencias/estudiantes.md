# Óxido para estudiantes

Óxido busca ofrecer una sintaxis bilingüe apoyándose en Rust. La meta es poder aprender estructuras de programación y, al mismo tiempo, encontrar las reglas reales de Rust para tipos, referencias, ownership y borrowing.

> Puedes ejecutar los ejemplos de Fases 5 y 7. Cada ejemplo enseña un subconjunto concreto; consulta la matriz antes de asumir que otra sintaxis está disponible.

## Programa indentado ejecutable

```ox
sintaxis indentacion
estructura Punto:
    x: i32
    y: i32

enumeracion Estado:
    Listo(i32)
```

El programa ejecutable completo está en `examples/fase_7_estructuras.ox`. Sus bloques usan indentación; tipos, expresiones y macros siguen las reglas Rust. `coincidir` conserva los brazos con llaves Rust.

Puedes ejecutarlo desde la raíz:

```sh
cargo run --locked -p oxido-cli -- ejecutar examples/fase_7_estructuras.ox
```

## Ruta de aprendizaje

1. Variables, expresiones y funciones.
2. Tipos y mutabilidad de Rust.
3. Condiciones y bucles.
4. Referencias, ownership y borrowing.
5. Errores del compilador y lectura del Rust generado.

`examples/fase_7_interoperabilidad.ox` muestra tuplas, referencias, métodos, módulos, errores `Result`/`Option` y `VecDeque` en sintaxis Rust. Consulta [modos de sintaxis](/referencia/syntax-modes/), [alias](/referencia/aliases/) y [compatibilidad](/referencia/compatibility/).
