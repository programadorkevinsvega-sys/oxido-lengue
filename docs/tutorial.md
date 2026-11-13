# Tutorial progresivo

Tres lecciones cortas para pasar de un programa mínimo a estructuras con métodos. Cada ejemplo está en `examples/tutorial/`, se ejecuta con la CLI actual y tiene una prueba automática que comprueba su salida. Son un subconjunto del lenguaje: no enseñan todo Rust ni sustituyen la [especificación](language-spec.md) ni la [matriz de compatibilidad](compatibility.md).

## Antes de empezar

Instala la CLI siguiendo la [guía de instalación](instalacion.md) o ejecuta desde la raíz del repositorio:

```sh
cargo run --locked -p oxido-cli -- ejecutar examples/tutorial/01_saludo.ox
```

Todos los comandos usan la misma forma. Si instalaste el binario, cambia `cargo run --locked -p oxido-cli --` por `oxido`.

## Lección 1: variables, funciones y salida

Archivo: `examples/tutorial/01_saludo.ox`

```ox
sintaxis indentacion

// Lección 1: variables, funciones y salida por consola.
funcion saludar(nombre: &texto) -> Cadena:
    formato!("Hola, {}!", nombre)

funcion principal():
    dejar nombre = "mundo"
    imprimir!("{}", saludar(nombre))
```

Salida esperada:

```text
Hola, mundo!
```

Qué observar:

- `sintaxis indentacion` declara el modo del archivo; los bloques se delimitan por dos puntos y sangría.
- `funcion principal()` es el punto de entrada, equivalente a `fn main`.
- `&texto` es una referencia a cadena y `Cadena` es el alias de `String`.
- `formato!` construye una cadena y `imprimir!` escribe en la salida estándar; los argumentos de las macros conservan Rust.
- `dejar` declara una variable inmutable. Para mutarla se usa `dejar mutable`.

**Tutorial 1.1:** cambia el nombre para que el programa salude a otra persona. Criterio: la salida cambia y el comando termina con código 0.

**Tutorial 1.2:** añade una segunda función que reciba dos números y devuelva su suma. Criterio: se llama desde `principal` y se imprime el resultado.

## Lección 2: condiciones, bucles y patrones

Archivo: `examples/tutorial/02_condiciones.ox`

```ox
sintaxis indentacion

// Lección 2: condiciones, bucles y coincidencia de patrones.
funcion clasificar(valor: i32) -> &'static str:
    si valor >= 10:
        "alto"
    sino:
        "bajo"

funcion principal():
    dejar valores = [3, 12, 5]
    para valor en valores:
        dejar etiqueta = coincidir valor:
            12 => "doce"
            _ => "otro"
        imprimir!("{}: {} ({})", valor, clasificar(valor), etiqueta)
```

Salida esperada:

```text
3: bajo (otro)
12: alto (doce)
5: bajo (otro)
```

Qué observar:

- `si`/`sino` son la forma indentada de `if`/`else`.
- `para ... en ...` recorre la colección; los arrays por valor funcionan como en Rust.
- `coincidir` es `match`; cada brazo usa `=>` y una expresión. `_` cubre el resto.
- La función devuelve la última expresión de cada rama, sin `return` ni `devolver`.

**Tutorial 2.1:** añade un tercer valor y una etiqueta nueva. Criterio: el brazo nuevo se ejecuta al menos una vez y los demás conservan su salida.

**Tutorial 2.2:** sustituye el array por un `Vector<i32>` y recorre su contenido con iteradores, como en `examples/estructuras.ox`. Criterio: la salida es equivalente y el programa compila.

## Lección 3: estructuras y métodos

Archivo: `examples/tutorial/03_punto.ox`

```ox
sintaxis indentacion

// Lección 3: estructuras, métodos y campos.
estructura Punto:
    x: i32
    y: i32

implementar Punto:
    funcion nuevo(x: i32, y: i32) -> Mismo:
        Mismo { x, y }
    funcion suma(&mismo) -> i32:
        mismo.x + mismo.y

funcion principal():
    dejar punto = Punto::nuevo(2, 3)
    imprimir!("({}, {}) suma {}", punto.x, punto.y, punto.suma())
```

Salida esperada:

```text
(2, 3) suma 5
```

Qué observar:

- `estructura` es `struct` y sus campos usan la misma sintaxis de tipos.
- `implementar` es `impl`.
- `Mismo` es el alias de `Self` en el modo indentado; `&mismo` es `&self`.
- Los campos se leen con `punto.x`; los métodos se llaman con `punto.suma()`.
- La inicialización compacta `Mismo { x, y }` conserva la semántica de Rust.

**Tutorial 3.1:** añade un campo `z` y actualiza el constructor y la suma. Criterio: compila y la salida incluye las tres coordenadas.

**Tutorial 3.2:** implementa un método que devuelva una referencia a un campo. Criterio: el tipo devuelto y el préstamo son válidos para `rustc`; si no, se explica el diagnóstico.

## Siguientes pasos

1. Usa `oxido exportar` para leer el Rust generado y compáralo con el `.ox`; los comandos están en la [guía de instalación](instalacion.md).
2. Consulta los [modos de sintaxis](syntax-modes.md) y los [alias](aliases.md) para escribir el mismo programa con nombres ingleses.
3. Revisa la [matriz de compatibilidad](compatibility.md) antes de usar una construcción que no aparezca en el tutorial.
4. Continúa con los [ejercicios y la ruta de aprendizaje](education.md).

## Límites

- El tutorial no cubre ownership, borrowing avanzado, traits, genéricos, módulos ni macros declarativas.
- No se ha validado en un aula ni con estudiantes; la sección de [educación](education.md) describe el plan de pilotos.
- Los ejemplos se prueban con el toolchain de CI; una versión distinta de Rust puede cambiar un diagnóstico, no la sintaxis del subconjunto.
