# Óxido para estudiantes

Óxido busca ofrecer una sintaxis bilingüe apoyándose en Rust. La meta es poder aprender estructuras de programación y, al mismo tiempo, encontrar las reglas reales de Rust para tipos, referencias, ownership y borrowing.

El material disponible es un subconjunto del lenguaje. Cada ejemplo enseña una parte concreta; consulta la [matriz de compatibilidad](../compatibility.md) antes de asumir que otra sintaxis está disponible.

## Empieza aquí

1. Sigue la [guía de instalación](../instalacion.md).
2. Ejecuta el [tutorial progresivo](../tutorial.md): tres lecciones con código y salida esperada.
3. Practica con el [libro de ejercicios](../ejercicios.md): los tres primeros incluyen archivo de inicio, solución y prueba automática.

## Programa indentado ejecutable

Óxido puede declarar estructuras y enumeraciones con indentación:

```ox
sintaxis indentacion

estructura Punto:
    x: i32
    y: i32

enumeracion Estado:
    Listo(i32)
```

El programa completo está en [`examples/estructuras.ox`](https://github.com/programadorkevinsvega-sys/oxido-lengue/blob/main/examples/estructuras.ox). Sus bloques usan indentación; tipos, expresiones y macros siguen las reglas Rust. `coincidir` conserva los brazos con llaves Rust.

Puedes ejecutarlo desde la raíz del repositorio:

```sh
cargo run --locked -p oxido-cli -- ejecutar examples/estructuras.ox
```

O, si instalaste el binario:

```sh
oxido ejecutar examples/estructuras.ox
```

## Ruta de aprendizaje

1. Variables, expresiones y funciones.
2. Tipos y mutabilidad de Rust.
3. Condiciones y bucles.
4. Referencias, ownership y borrowing.
5. Errores del compilador y lectura del Rust generado.

El ejemplo [`examples/interoperabilidad.ox`](https://github.com/programadorkevinsvega-sys/oxido-lengue/blob/main/examples/interoperabilidad.ox) muestra tuplas, referencias, métodos, módulos, errores `Result`/`Option` y `VecDeque` en sintaxis Rust. Consulta [modos de sintaxis](../syntax-modes.md), [alias](../aliases.md), [compatibilidad](../compatibility.md) y la [guía de educación](../education.md).
