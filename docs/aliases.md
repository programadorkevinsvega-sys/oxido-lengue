# Alias, keywords e identificadores

## Reglas

- Rust es sensible a mayúsculas: `mismo` y `Mismo` representan `self` y `Self` respectivamente.
- Los alias son tokens de keyword solo en forma no escapada y en contexto de código.
- Literales, comentarios, documentación, atributos desconocidos y árboles de argumentos de macro se preservan.
- Los aliases del prelude solo se traducen en tokens no crudos; usa `r#Cadena`, por ejemplo, para un identificador propio con ese nombre.
- `r#nombre` fuerza un identificador literal llamado `nombre`, sin alias. Ejemplo: `r#tipo` es una variable llamada `tipo`; no es el keyword `type`.
- Si se usa un alias reservado sin escape como identificador, emitir `OX3001`: «`tipo` es palabra reservada de Óxido; usa `r#tipo` o cámbialo».
- La forma cruda respeta restricciones de identificadores Rust; Rust no permite `r#_`, `r#crate`, `r#self`, `r#Self` ni `r#super` ([Rust Reference: Identifiers](https://doc.rust-lang.org/reference/identifiers.html#raw-identifiers)).

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
| `let` | `dejar`, `variable` |
| `loop` | `bucle` |
| `match` | `coincidir` |
| `mod` | `modulo` |
| `move` | `mover` |
| `mut` | `mutable` |
| `pub` | `publico` |
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
| `_` | `_` (sin traducción) |

## Keywords reservadas sin semántica Óxido

Estas formas siguen reservadas y no se convierten en características. Su uso como sintaxis produce `OX4001` (construcción Rust reservada/no soportada); como identificador requieren `r#` cuando Rust lo permita.

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

Rust 2024 reserva `gen`; el inventario debe actualizarse al cambiar la edición objetivo ([Rust Reference: Keywords](https://doc.rust-lang.org/reference/keywords.html#reserved-keywords)).

## Keywords débiles/contextuales conservadas

Se mantienen en inglés y con reglas contextuales de Rust: `'static`, `macro_rules`, `raw`, `safe`, `union`. No se les asigna alias español en 0.1.

## Macros y atributos

Los nombres allowlisted de macros se traducen; sus argumentos nunca se traducen. Por tanto, los argumentos deben usar sintaxis Rust (`vector![String::from(...)]`, `formato!("{}", valor)`). `matches!` y macros de terceros mantienen nombre y argumentos intactos. Lista inicial: `imprimirln!`→`println!`, `imprimir!`→`print!`, `formato!`→`format!`, `vector!`→`vec!`, `panico!`→`panic!`, `afirmar!`→`assert!`, `afirmar_igual!`→`assert_eq!`, `pendiente!`→`todo!`, `no_implementado!`→`unimplemented!`, `inalcanzable!`→`unreachable!`, `depurar!`→`dbg!`.

Solo se traducen estos atributos: `#[prueba]`→`#[test]`, `#[permitir(...)]`→`#[allow(...)]`, `#[derivar(...)]`→`#[derive(...)]`. Dentro de `derivar`, también se normalizan los aliases de traits de la tabla. Todo atributo no listado permanece opaco.

Rust especifica una invocación como path, `!` y árbol delimitado ([Rust Reference: Macro Invocation](https://doc.rust-lang.org/reference/macros.html#macro-invocation)).

## Prelude español inicial

| Óxido | Rust | Alcance |
|---|---|---|
| `Cadena`, `Vector`, `Opcion`, `Alguno`, `Ninguno`, `Resultado`, `Bien`, `Error`, `Caja`, `MapaHash` | `String`, `Vec`, `Option`, `Some`, `None`, `Result`, `Ok`, `Err`, `Box`, `HashMap` | Identificadores fuera de macros/atributos |
| `booleano`, `caracter`, `texto` | `bool`, `char`, `str` | Identificadores de tipo; `texto` se escribe como `&texto` para un préstamo textual |
| `Defecto`, `Iterador` | `Default`, `Iterator` | Traits/derives |

Los nombres numéricos (`i32`, `u8`, `f64`) y los traits `Display`, `Debug`, `Clone`, `Copy`, `From` e `Into` se conservan. También se reconocen métodos `desenvolver`, `esperar_o`, `agregar`, `largo`, `iterar`, `mapear`, `filtrar`, `recolectar`, `clonar`, `a_cadena` y `es_alguno` como aliases de `unwrap`, `expect`, `push`, `len`, `iter`, `map`, `filter`, `collect`, `clone`, `to_string` e `is_some`; se aplican a llamadas con punto y declaraciones de método. Los módulos `std::colecciones`, `std::archivos`, `std::es` y `std::hilo` se normalizan únicamente bajo `std::`.

`principal` solo nombra la función de entrada y se genera como `main`; no puede usarse como otro identificador salvo con escape raw cuando Rust lo permita. `variable mutable x` equivale a `let mut x`; `variable x` equivale a `let x`, y `mutable` también es el alias independiente de `mut` en referencias y parámetros.
