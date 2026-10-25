# Alias, keywords e identificadores

## Reglas

- Rust es sensible a mayúsculas: `mismo` y `Mismo` representan `self` y `Self` respectivamente.
- Los alias son tokens de keyword solo en forma no escapada y en contexto de código.
- Literales, comentarios, documentación y atributos desconocidos se preservan. Los argumentos de macros solo se recorren en la lista de macros estándar que acepta expresiones.
- Los aliases del prelude solo se traducen en tokens no crudos; usa `r#Cadena`, por ejemplo, para un identificador propio con ese nombre.
- `r#nombre` fuerza un identificador literal llamado `nombre`, sin alias. Ejemplo: `r#tipo` es una variable llamada `tipo`; no es el keyword `type`.
- Si se usa un alias reservado sin escape como identificador, emitir `OX3001`: «`tipo` es palabra reservada de Óxido; usa `r#tipo` o cámbialo».
- La forma cruda respeta restricciones de identificadores Rust; Rust no permite `r#_`, `r#crate`, `r#self`, `r#Self` ni `r#super` ([Rust Reference: Identifiers](https://doc.rust-lang.org/reference/identifiers.html#raw-identifiers)).

<!-- BEGIN GENERATED ALIASES -->
## Keywords estrictas

| Rust 2024 | Alias Óxido |
|---|---|
| `as` | `como` |
| `async` | `asincrono` |
| `await` | `esperar` |
| `break` | `romper` |
| `const` | `constante` |
| `continue` | `continuar` |
| `crate` | `paquete` |
| `dyn` | `dinamico` |
| `else` | `sino` |
| `enum` | `enumeracion` |
| `extern` | `externo` |
| `false` | `falso` |
| `fn` | `funcion` |
| `for` | `para` |
| `if` | `si` |
| `impl` | `implementar` |
| `in` | `en` |
| `let` | `dejar` |
| `loop` | `bucle` |
| `match` | `coincidir` |
| `mod` | `modulo` |
| `move` | `mover` |
| `mut` | `mutable` |
| `pub` | `publico` |
| `main` | `principal` |
| `ref` | `referencia` |
| `return` | `retornar` |
| `self` | `mismo` |
| `Self` | `Mismo` |
| `static` | `estatico` |
| `struct` | `estructura` |
| `super` | `superior` |
| `trait` | `rasgo` |
| `true` | `verdadero` |
| `type` | `tipo` |
| `unsafe` | `inseguro` |
| `use` | `usar` |
| `where` | `donde` |
| `while` | `mientras` |

## Keywords reservadas sin semántica Óxido

| Rust 2024 | Alias reservado |
|---|---|
| `abstract` | `abstracto` |
| `become` | `convertirse` |
| `box` | `caja` |
| `do` | `hacer` |
| `final` | `final` |
| `gen` | `generar` |
| `macro` | `macro` |
| `override` | `sobrescribir` |
| `priv` | `privado` |
| `try` | `intentar` |
| `typeof` | `tipo_de` |
| `unsized` | `sin_tamano` |
| `virtual` | `virtual` |
| `yield` | `ceder` |

## Macros estándar

| Óxido | Rust |
|---|---|
| `imprimir!` | `println!` |
| `imprimir_sin_salto!` | `print!` |
| `imprimir_error!` | `eprintln!` |
| `imprimir_error_sin_salto!` | `eprint!` |
| `formato!` | `format!` |
| `argumentos_formato!` | `format_args!` |
| `escribir!` | `write!` |
| `escribir_linea!` | `writeln!` |
| `vector!` | `vec!` |
| `panico!` | `panic!` |
| `afirmar!` | `assert!` |
| `afirmar_igual!` | `assert_eq!` |
| `afirmar_distinto!` | `assert_ne!` |
| `afirmar_debug!` | `debug_assert!` |
| `afirmar_igual_debug!` | `debug_assert_eq!` |
| `afirmar_distinto_debug!` | `debug_assert_ne!` |
| `depurar!` | `dbg!` |
| `pendiente!` | `todo!` |
| `sin_implementar!` | `unimplemented!` |
| `inalcanzable!` | `unreachable!` |
| `coincide!` | `matches!` |
| `incluir_texto!` | `include_str!` |
| `incluir_bytes!` | `include_bytes!` |
| `entorno!` | `env!` |
| `entorno_opcional!` | `option_env!` |
| `concatenar!` | `concat!` |
| `a_texto!` | `stringify!` |
| `linea!` | `line!` |
| `archivo!` | `file!` |
| `columna!` | `column!` |
| `config!` | `cfg!` |
| `error_de_compilacion!` | `compile_error!` |
| `local_del_hilo!` | `thread_local!` |

## Tipos y variantes del prelude

| Óxido | Rust |
|---|---|
| `Cadena` | `String` |
| `Vector` | `Vec` |
| `Opcion` | `Option` |
| `Alguno` | `Some` |
| `Ninguno` | `None` |
| `Resultado` | `Result` |
| `Bien` | `Ok` |
| `Falla` | `Err` |
| `Caja` | `Box` |
| `MapaHash` | `HashMap` |
| `booleano` | `bool` |
| `caracter` | `char` |
| `Defecto` | `Default` |
| `Iterador` | `Iterator` |
| `texto` (solo tipo) | `str` |

### Métodos del prelude

| Óxido | Rust |
|---|---|
| `Vector::nuevo()` | `Vec::new()` |
| `.esta_vacio()` | `.is_empty()` |
| `.agregar()` | `.push()` |
| `.sacar()` | `.pop()` |
| `.obtener()` | `.get()` |
| `.insertar()` | `.insert()` |
| `.quitar()` | `.remove()` |
| `.contiene()` | `.contains()` |
| `.vaciar()` | `.clear()` |
| `.largo()` | `.len()` |
| `.iterar()` | `.iter()` |
| `.iterar_mutable()` | `.iter_mut()` |
| `.mapear()` | `.map()` |
| `.filtrar()` | `.filter()` |
| `.recolectar()` | `.collect()` |
| `.encontrar()` | `.find()` |
| `.sumar()` | `.sum()` |
| `.ordenar()` | `.sort()` |
| `.clonar()` | `.clone()` |
| `.desenvolver()` | `.unwrap()` |
| `.exigir()` | `.expect()` |
| `.es_alguno()` | `.is_some()` |
| `.es_ninguno()` | `.is_none()` |
| `.a_mayusculas()` | `.to_uppercase()` |
| `.a_minusculas()` | `.to_lowercase()` |
| `.a_cadena()` | `.to_string()` |

### Módulos de `std`

| Óxido | Rust |
|---|---|
| `colecciones` (tras `std::`) | `collections` |
| `archivos` (tras `std::`) | `fs` |
| `es` (tras `std::`) | `io` |
| `hilo` (tras `std::`) | `thread` |

### Atributos

| Óxido | Rust |
|---|---|
| `#[prueba]` | `#[test]` |
| `#[permitir]` | `#[allow]` |
| `#[derivar]` | `#[derive]` |
<!-- END GENERATED ALIASES -->

## Keywords débiles/contextuales conservadas

Se mantienen en inglés y con reglas contextuales de Rust: `'static`, `macro_rules`, `raw`, `safe`, `union`. No se les asigna alias español en 0.1.

## Macros y atributos

Se traduce el nombre de una macro estándar allowlisted; los nombres Rust originales siguen válidos. `std::alias!` también se normaliza. Los paths de otros crates (`serde_json::json!`, `sqlx::query!`) y las macros desconocidas conservan su nombre y su árbol de argumentos.

Para macros estándar que reciben expresiones (`print*`, `format*`, `write*`, `vec!`, `panic!`, `assert*`, `dbg!`, `todo!`, `unimplemented!`, `unreachable!` y `matches!`), el backend recorre tokens de expresiones para normalizar aliases y macros estándar anidadas. Las cadenas y literales no se modifican. Macros de metaprogramación/configuración como `concat!`, `stringify!`, `cfg!`, `include_*`, `env!`, `line!`, `file!`, `column!`, `compile_error!` y `thread_local!` traducen solo el nombre y mantienen sus argumentos opacos.

Los argumentos de las macros externas son opacos intencionalmente: Óxido no debe cambiar DSLs como JSON o SQL ni exigir que sus dependencias adopten aliases españoles. La expansión y la semántica siguen correspondiendo a Rust.

Solo se traducen los atributos enumerados en la tabla generada. Dentro de `derivar`, también se normalizan los aliases de traits registrados. Todo atributo no listado permanece opaco.

Rust especifica una invocación como path, `!` y árbol delimitado ([Rust Reference: Macro Invocation](https://doc.rust-lang.org/reference/macros.html#macro-invocation)).

Los nombres numéricos (`i32`, `u8`, `f64`) y los traits `Display`, `Debug`, `Clone`, `Copy`, `From` e `Into` se conservan. Los aliases de métodos se normalizan en llamadas y `nuevo` también en constructores del prelude; las declaraciones `fn` permanecen intactas para que las implementaciones de traits conserven sus nombres Rust. El traductor recoge las funciones y métodos declarados en el archivo `.ox` compilado: si uno coincide con un alias, preserva esas llamadas y emite `OX3002`; escribe el nombre Rust (`.get()`) para llamar al método estándar de ese mismo nombre. El compilador actual procesa un archivo fuente `.ox` por invocación, así que el conjunto cubre esa unidad de compilación. Los nombres de módulos `es`, `hilo`, `archivos` y `colecciones` solo se traducen tras `std::`; como nombres sueltos se conservan. Las rutas y macros externas se mantienen intactas.

`principal` solo nombra la función de entrada y se genera como `main`; no puede usarse como otro identificador salvo con escape raw cuando Rust lo permita. `dejar mutable x` equivale a `let mut x`; `dejar x` equivale a `let x`, y `mutable` también es el alias independiente de `mut` en referencias y parámetros.
