# English quickstart

This is the maintained English quickstart for Óxido. The canonical technical documentation is written in Spanish; this page covers installation, the first program and the progressive tutorial, and links to the Spanish reference where needed.

> **Language status:** only this quickstart is maintained in English. The rest of the site is Spanish. The mode header is Spanish-only today: `sintaxis indentacion` or `sintaxis llaves`. Rust keywords such as `fn`, `let`, `struct`, `impl`, `self` and `Self` work unchanged inside that mode.

## Requirements

- Rust 1.99.1 or newer and Cargo.
- `rustfmt` from the same toolchain. Without it, generation fails with `OX4001`.
- Optional: VS Code and `oxido-lsp`. See the [Spanish editor guide](../vscode.md).

```sh
rustc --version
cargo --version
rustfmt --version
```

## Install from source

From the repository root:

```sh
cargo install --path crates/oxido-cli --locked
oxido --help
```

`cargo install` builds in release mode and publishes nothing. The crate has `publish = false`.

## First program

Create `hello.ox`:

```ox
sintaxis indentacion

fn main():
    let name = "world";
    println!("Hello, {}!", name);
```

Run it:

```sh
oxido ejecutar hello.ox
```

Expected output:

```text
Hello, world!
```

The first line selects indentation mode. Blocks end with `:` and indentation. The body uses standard Rust keywords, so `fn`, `let` and `println!` need no translation.

## Same program, Spanish keywords

Óxido accepts Spanish aliases without changing semantics:

```ox
sintaxis indentacion

funcion principal():
    dejar nombre = "mundo";
    imprimir!("Hola, {}!", nombre);
```

Both files produce the same Rust structure. The [alias reference](../aliases.md) lists the current translations and the prelude types.

## Structs and methods

```ox
sintaxis indentacion

struct Punto:
    x: i32
    y: i32

impl Punto:
    fn nuevo(x: i32, y: i32) -> Self:
        Self { x, y }
    fn suma(&self) -> i32:
        self.x + self.y

fn main():
    let punto = Punto::nuevo(2, 3);
    println!("({}, {}) suma {}", punto.x, punto.y, punto.suma());
```

Expected output:

```text
(2, 3) suma 5
```

## Progressive tutorial

The [Spanish tutorial](../tutorial.md) has three runnable lessons. The examples live in `examples/tutorial/`:

```sh
oxido ejecutar examples/tutorial/01_saludo.ox
oxido ejecutar examples/tutorial/02_condiciones.ox
oxido ejecutar examples/tutorial/03_punto.ox
```

These examples use Spanish keywords; each has a test that compiles and runs it in CI.

## Commands

| Command | Purpose | Current limit |
|---|---|---|
| `nuevo <dir>` | Create a minimal project | Fixed indentation template |
| `formato <file.ox>` | Format and check idempotence | Only the documented subset |
| `exportar <file.ox> --salida <file.rs>` | Generate Rust and a source map | Final Rust is formatted with `rustfmt` |
| `check <file.ox>` (`verificar`) | Validate generated Rust with `cargo check`, without leaving an executable | Requires `rustfmt`; dependencies need a lockfile |
| `build <file.ox>` (`compilar`) | Generate a persistent Cargo project in `target/oxido/` and a binary | `--release` selects Cargo's optimized profile; binary output is under `target/oxido/target/{debug,release}` |
| `run <file.ox>` (`ejecutar`) | Compile and run the program | `--release` selects Cargo's optimized profile |
| `test <file.ox>` (`probar`) | Run `cargo test` | A file without `#[test]` reports zero tests |

See the [Spanish installation guide](../instalacion.md) for dependencies, troubleshooting and uninstall steps.

## Limits

- The language subset is defined by the [compatibility matrix](../compatibility.md). A program that compiles does not prove general Rust compatibility.
- The CLI does not sandbox dependency code. Compiling or running with dependencies requires `--confiar-dependencias` and may execute `build.rs` and procedural macros.
- The VS Code extension is not published or signed, and only the Linux x64 package with the server has been run end to end.
- The playground sandbox is Linux-only and is not deployed.
- This page is not a translation of the whole site. If the Spanish documentation changes, only the content covered here is updated.

## License

The code is dual-licensed under MIT or Apache-2.0. See [`LICENSE-MIT`](https://github.com/programadorkevinsvega-sys/oxido-lengue/blob/main/LICENSE-MIT) and [`LICENSE-APACHE`](https://github.com/programadorkevinsvega-sys/oxido-lengue/blob/main/LICENSE-APACHE).
