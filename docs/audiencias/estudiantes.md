# Óxido para estudiantes

Óxido busca ofrecer una sintaxis bilingüe apoyándose en Rust. La meta es poder aprender estructuras de programación y, al mismo tiempo, encontrar las reglas reales de Rust para tipos, referencias, ownership y borrowing.

El material disponible es un subconjunto del lenguaje. Cada ejemplo enseña una parte concreta; consulta la [matriz de compatibilidad](../compatibility.md) antes de asumir que otra sintaxis está disponible.

## Empieza aquí

1. Instala el paquete Debian desde la [guía de instalación](../instalacion.md); no tienes que clonar el repositorio.
2. Crea un proyecto con `oxido nuevo curso-oxido` y entra en su carpeta. Si no tienes Rust 1.99.1 o posterior, instala la toolchain aislada con `sudo oxido-instalar-toolchain`.
3. Copia una lección del [tutorial progresivo](../tutorial.md) a `src/main.ox` y ejecútala con `oxido ejecutar src/main.ox`.
4. Practica con el [libro de ejercicios](../ejercicios.md). Sus enunciados se resuelven en tu proyecto local.

## Programa indentado ejecutable

Óxido puede declarar estructuras y enumeraciones con indentación. Para probar este ejemplo con la CLI instalada, crea el proyecto y pega el código completo en `curso-oxido/src/main.ox`:

```ox
sintaxis indentacion

estructura Punto:
    x: i32
    y: i32

enumeracion Estado:
    Listo(i32)

usar std::collections::VecDeque

funcion main():
    dejar punto = Punto { x: 2, y: 3 }
    dejar estado = Estado::Listo(punto.x + punto.y)
    dejar valor = coincidir estado { Estado::Listo(numero) => numero, }
    dejar mut cola = VecDeque::from([1, 2, 3])
    cola.push_back(4)
    println!("{}:{}", valor, cola.pop_front().unwrap());
```

El programa es el fixture [`examples/estructuras.ox`](https://github.com/programadorkevinsvega-sys/oxido-lengue/blob/main/examples/estructuras.ox), con su contenido completo; ese archivo no viene dentro del `.deb`. Sus bloques usan indentación; tipos, expresiones y macros siguen las reglas Rust. `coincidir` conserva los brazos con llaves Rust. La salida es `5:1`.

Ejecuta tu copia local del proyecto:

```sh
cd curso-oxido
oxido ejecutar src/main.ox
```

Los comandos para clonar el repositorio y ejecutar directamente sus fixtures están en [Clonar, compilar y contribuir](../contribuir.md).

## Ruta de aprendizaje

1. Variables, expresiones y funciones.
2. Tipos y mutabilidad de Rust.
3. Condiciones y bucles.
4. Referencias, ownership y borrowing.
5. Errores del compilador y lectura del Rust generado.

El ejemplo [`examples/interoperabilidad.ox`](https://github.com/programadorkevinsvega-sys/oxido-lengue/blob/main/examples/interoperabilidad.ox) muestra tuplas, referencias, métodos, módulos, errores `Result`/`Option` y `VecDeque` en sintaxis Rust. Consulta [modos de sintaxis](../syntax-modes.md), [alias](../aliases.md), [compatibilidad](../compatibility.md) y la [guía de educación](../education.md).
