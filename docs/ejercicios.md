# Ejercicios y criterios de evaluación

Libro de ejercicios para practicar el subconjunto documentado de Óxido. Los tres primeros tienen un archivo de inicio, una solución de referencia y una prueba automática que comprueba la salida. El resto son propuestas con criterios observables y **no** se han validado con estudiantes.

## Estado

- **Verificado técnicamente:** las soluciones de los ejercicios 1–3 compilan y se ejecutan en CI. Sus salidas están fijadas en las pruebas.
- **Propuesto:** los ejercicios 4–9, la rúbrica y el registro de evidencia. No hay datos de aula.
- **Fuera de alcance:** corrección automática en red, cuentas de usuario y telemetría educativa.

Empieza por el [tutorial progresivo](tutorial.md) si aún no ejecutaste un programa. Para el protocolo de un piloto real, consulta [Pilotos educativos](pilotos-educativos.md).

## Cómo usar el libro

1. Descarga el archivo de inicio del repositorio o cópialo a un directorio de trabajo.
2. Ejecuta `oxido ejecutar archivo.ox` y compara con el criterio.
3. Consulta la solución solo después de intentarlo.
4. Anota el diagnóstico completo si el programa no compila.

Los archivos de inicio compilan, pero emiten avisos de variables o campos sin usar y devuelven un resultado incompleto; es esperado hasta completar el ejercicio.

Los archivos viven en [`examples/ejercicios/`](https://github.com/programadorkevinsvega-sys/oxido-lengue/tree/main/examples/ejercicios). Cada ejercicio enlaza al archivo de inicio y a su solución en el repositorio:

| Ejercicio | Inicio | Solución | Salida esperada |
|---|---|---|---|
| 1. Promedio | [`01_promedio.ox`](https://github.com/programadorkevinsvega-sys/oxido-lengue/blob/main/examples/ejercicios/01_promedio.ox) | [`soluciones/01_promedio.ox`](https://github.com/programadorkevinsvega-sys/oxido-lengue/blob/main/examples/ejercicios/soluciones/01_promedio.ox) | `8.00` |
| 2. Inventario | [`02_inventario.ox`](https://github.com/programadorkevinsvega-sys/oxido-lengue/blob/main/examples/ejercicios/02_inventario.ox) | [`soluciones/02_inventario.ox`](https://github.com/programadorkevinsvega-sys/oxido-lengue/blob/main/examples/ejercicios/soluciones/02_inventario.ox) | `te: 6`, `cafe: 7`, `total: 13` |
| 3. Palabras largas | [`03_palabras.ox`](https://github.com/programadorkevinsvega-sys/oxido-lengue/blob/main/examples/ejercicios/03_palabras.ox) | [`soluciones/03_palabras.ox`](https://github.com/programadorkevinsvega-sys/oxido-lengue/blob/main/examples/ejercicios/soluciones/03_palabras.ox) | `1` |

## Nivel 1: fundamentos

### 1. Promedio de un array

Archivo de inicio: [`examples/ejercicios/01_promedio.ox`](https://github.com/programadorkevinsvega-sys/oxido-lengue/blob/main/examples/ejercicios/01_promedio.ox)

```ox
sintaxis indentacion

// Ejercicio 1 (inicio): completa el cálculo del promedio.
// Criterio: la salida debe ser 8.00 para [8, 6, 10].
funcion promedio(valores: &[i32]) -> f64:
    // TODO: suma los valores y divide entre su cantidad.
    0.0

funcion principal():
    dejar valores = [8, 6, 10]
    imprimir!("{:.2}", promedio(&valores))
```

- **Tarea:** sumar los valores y dividir entre la cantidad, sin división entera.
- **Conceptos:** `para`, `mutable`, conversión `como f64`, formato `{:.2}`.
- **Criterio:** la salida es exactamente `8.00` y el proceso termina con código 0.
- **Límite:** no se evalúa redondeo ni precisión más allá de dos decimales.
- **Solución:** [`soluciones/01_promedio.ox`](https://github.com/programadorkevinsvega-sys/oxido-lengue/blob/main/examples/ejercicios/soluciones/01_promedio.ox).

### 2. Inventario con estructuras

Archivo de inicio: [`examples/ejercicios/02_inventario.ox`](https://github.com/programadorkevinsvega-sys/oxido-lengue/blob/main/examples/ejercicios/02_inventario.ox)

```ox
sintaxis indentacion

// Ejercicio 2 (inicio): completa el inventario.
// Criterio: imprimir cada producto y un total de 13.
estructura Producto:
    nombre: Cadena
    cantidad: i32
    precio: i32

implementar Producto:
    funcion subtotal(&mismo) -> i32:
        0

funcion principal():
    dejar productos = vector![
        Producto { nombre: "te".a_cadena(), cantidad: 2, precio: 3 },
        Producto { nombre: "cafe".a_cadena(), cantidad: 1, precio: 7 },
    ]
    // TODO: recorre los productos, imprime "nombre: subtotal" y suma el total.
    imprimir!("total: {}", 0)
```

- **Tarea:** implementar `subtotal` y recorrer el vector para imprimir cada producto y el total.
- **Conceptos:** `estructura`, `implementar`, `&mismo`, `Vector`, `iter()`.
- **Criterio:** se imprimen tres líneas y el total es `13`.
- **Límite:** el orden de los productos debe conservarse; no se permiten dependencias externas.
- **Solución:** [`soluciones/02_inventario.ox`](https://github.com/programadorkevinsvega-sys/oxido-lengue/blob/main/examples/ejercicios/soluciones/02_inventario.ox).

### 3. Contar palabras largas

Archivo de inicio: [`examples/ejercicios/03_palabras.ox`](https://github.com/programadorkevinsvega-sys/oxido-lengue/blob/main/examples/ejercicios/03_palabras.ox)

```ox
sintaxis indentacion

// Ejercicio 3 (inicio): cuenta palabras largas.
// Criterio: con ["oxido", "rust", "metal", "aprender"] y mínimo 6, la salida es 1.
funcion contar_largas(palabras: &[&str], minimo: usize) -> usize:
    // TODO: usa iter(), filter() y count().
    0

funcion principal():
    dejar palabras = ["oxido", "rust", "metal", "aprender"]
    imprimir!("{}", contar_largas(&palabras, 6))
```

- **Tarea:** contar las palabras con longitud mayor o igual al mínimo usando iteradores.
- **Conceptos:** `iter()`, `filter()`, `count()`, closures y `largo()`.
- **Criterio:** con el array dado y mínimo 6, la salida es `1`.
- **Límite:** no se evalúa el rendimiento; la solución debe ser funcional.
- **Solución:** [`soluciones/03_palabras.ox`](https://github.com/programadorkevinsvega-sys/oxido-lengue/blob/main/examples/ejercicios/soluciones/03_palabras.ox).

## Nivel 2: colecciones y control de flujo

### 4. FizzBuzz acotado

- **Tarea:** imprimir los números del 1 al 20; para múltiplos de 3, `Fizz`; de 5, `Buzz`; de ambos, `FizzBuzz`.
- **Conceptos:** `para`, `si`/`sino`, módulo `%`, comparaciones.
- **Criterio:** 20 líneas, sin números fuera de rango y sin pánicos.
- **Límite:** el formato exacto lo define el docente; se documenta antes de evaluar.

### 5. Buscar el máximo

- **Tarea:** devolver el mayor valor de un `Vector<i32>` sin usar `max()`.
- **Conceptos:** iteración, mutabilidad, comparaciones, `Option` si se decide devolver `None` para un vector vacío.
- **Criterio:** cubre vector con positivos, negativos, un solo elemento y vacío según el contrato elegido.
- **Límite:** el caso vacío debe estar decidido y probado, no ignorado.

### 6. Contar vocales

- **Tarea:** contar vocales en una cadena, incluidas mayúsculas y vocales acentuadas si el docente lo pide.
- **Conceptos:** `chars()`, `filter()`, comparaciones, Unicode.
- **Criterio:** los casos de prueba acordados dan el conteo esperado.
- **Límite:** el tratamiento de Unicode debe documentarse; no se asume que `len()` sea el número de caracteres.

## Nivel 3: errores, módulos y Rust generado

### 7. Result y propagación

- **Tarea:** convertir una cadena a número con `parse` y devolver un `Result` con mensaje propio.
- **Conceptos:** `Result`, `?`, `map_err`, `ok_or`.
- **Criterio:** una entrada válida produce `Ok`; una inválida produce un error controlado sin pánico.
- **Límite:** el mensaje de error es parte del contrato y debe documentarse.

### 8. Módulos y visibilidad

- **Tarea:** separar una función de cálculo en un módulo y exponerla con `publico`.
- **Conceptos:** `modulo`, `publico`, rutas, `usar`.
- **Criterio:** el programa compila y solo la función prevista es visible desde `principal`.
- **Límite:** la importación de un árbol completo de archivos `.ox` sigue siendo un límite documentado.

### 9. Leer el Rust generado

- **Tarea:** exportar un programa y localizar en el `.rs` tres construcciones: función, estructura y macro.
- **Conceptos:** `exportar`, mapa de origen, `rustfmt`, frontera frontend/Rust.
- **Criterio:** el estudiante cita la línea del `.rs` y explica qué transformó Óxido y qué conservó Rust.
- **Límite:** la comparación es estructural; el texto no es idéntico.

## Rúbrica sugerida

| Nivel | Descripción | Puntos |
|---|---|---|
| Ejecuta | El programa termina con código 0 y sin pánicos | 1 |
| Cumple | La salida coincide con el criterio documentado | 2 |
| Explica | El estudiante describe el diagnóstico o el Rust generado | 1 |
| Prueba | Añade al menos un caso límite propio | 1 |

Una solución que compila pero no cumple el criterio no se considera completa. Una solución copiada de la referencia sin explicación puede obtener el nivel «ejecuta», no «explica».

## Registro de evidencia para un piloto

Si se usa en un aula, registra solo datos agregados y con consentimiento:

- número de participantes por grupo;
- ejercicios intentados y completados;
- tiempo hasta la primera ejecución correcta;
- diagnósticos `OX*` y de `rustc` más frecuentes;
- casos límite añadidos por los estudiantes;
- incidencias de instalación por plataforma.

No registres nombres, rutas personales ni identificadores. El protocolo completo está en [Pilotos educativos](pilotos-educativos.md).

## Límites

- Los ejercicios 1–3 están verificados técnicamente; no se ha medido su dificultad real.
- Los ejercicios 4–9 son propuestas y pueden necesitar ajustes tras una prueba con docentes.
- La matriz de compatibilidad delimita la sintaxis; un ejercicio que use otra construcción debe marcarse como fuera de alcance.
