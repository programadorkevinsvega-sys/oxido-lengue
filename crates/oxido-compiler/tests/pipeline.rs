use oxido_language::SourceFile;

#[test]
fn parses_formats_and_reparses_a_rust_file() {
    let source = SourceFile::new("fn suma(a:i32,b:i32)->i32{a+b}");
    let analysis = oxido_compiler::analyze_rust(&source).expect("pipeline Rust válido");
    assert_eq!(analysis.item_count, 1);
}

#[test]
fn generates_a_rust_program_with_a_serializable_source_map() {
    use oxido_language::domain::syntax::LexOptions;

    let source =
        SourceFile::new("sintaxis indentacion\nfuncion main():\n    println!(\"Hola, mundo.\");\n");
    let generated = oxido_compiler::generate_ox(&source, LexOptions::default())
        .unwrap_or_else(|diagnostics| panic!("{diagnostics:?}"));
    assert!(generated.rust.contains("fn main()"));
    assert!(generated.rust.contains("println!(\"Hola, mundo.\")"));
    oxido_compiler::analyze_rust(&SourceFile::new(&generated.rust)).unwrap();

    let mapped = generated
        .source_map
        .map_generated(2, 5)
        .expect("el call site de la macro debe volver al .ox");
    let original = source.span(mapped.start, mapped.end).unwrap();
    assert!(source.slice(original).contains("println!"));
    let serialized = serde_json::to_string(&generated.source_map).unwrap();
    assert!(serialized.contains("generatedStartLine"));
}

#[test]
fn preserves_a_final_control_expression_as_the_rust_block_value() {
    use oxido_language::domain::syntax::LexOptions;

    let source = SourceFile::new(
        "sintaxis llaves\nfn elegir(condicion: bool) -> i32 { if condicion { 1 } else { 2 } }\n",
    );
    let generated = oxido_compiler::generate_ox(&source, LexOptions::default())
        .unwrap_or_else(|diagnostics| panic!("{diagnostics:?}"));
    assert!(generated.rust.contains("else"));
    assert!(!generated.rust.contains("};"));
    oxido_compiler::analyze_rust(&SourceFile::new(&generated.rust)).unwrap();
}

#[test]
fn compiles_and_runs_a_minimal_ox_program_through_cargo() {
    use oxido_compiler::application::cargo::CargoAction;
    use oxido_language::domain::syntax::LexOptions;

    let source = SourceFile::new(
        "sintaxis indentacion\nfuncion main():\n    dejar valor = 40 + 2\n    println!(\"Hola, {}.\", valor);\n",
    );
    let checked = oxido_compiler::build_ox(&source, LexOptions::default(), CargoAction::Check)
        .unwrap_or_else(|diagnostics| panic!("{diagnostics:?}"));
    assert!(
        checked.success,
        "{} {:?}",
        checked.stderr, checked.diagnostics
    );

    let run = oxido_compiler::build_ox(&source, LexOptions::default(), CargoAction::Run)
        .unwrap_or_else(|diagnostics| panic!("{diagnostics:?}"));
    assert!(run.success, "{} {:?}", run.stderr, run.diagnostics);
    assert_eq!(run.program_output, ["Hola, 42."]);
}

#[test]
fn differential_indented_ox_and_equivalent_rust_have_the_same_observable_result() {
    assert_ox_and_rust_output_match(
        "sintaxis indentacion\nfuncion clasificar(valor: i32) -> &'static str:\n    si valor >= 10:\n        \"alto\"\n    sino:\n        \"bajo\"\nfuncion main():\n    dejar valores = vector![3, 12, 5]\n    dejar resultado = valores.iter().map(|valor| clasificar(*valor)).collect::<Vector<_>>()\n    println!(\"{}\", resultado.join(\",\"));\n",
        r#"fn clasificar(valor: i32) -> &'static str {
    if valor >= 10 { "alto" } else { "bajo" }
}
fn main() {
    let valores = vec![3, 12, 5];
    let resultado = valores.iter().map(|valor| clasificar(*valor)).collect::<Vec<_>>();
    println!("{}", resultado.join(","));
}
"#,
    );
}

#[test]
fn differential_indented_impl_and_match_have_the_same_result_as_rust() {
    assert_ox_and_rust_output_match(
        r#"sintaxis indentacion
estructura Punto:
    x: i32
    y: i32
implementar Punto:
    funcion cuadrado(&mismo) -> i32:
        mismo.x * mismo.x + mismo.y * mismo.y
funcion principal():
    dejar punto = Punto { x: 3, y: 4 }
    coincidir punto.cuadrado():
        25 => imprimir!("25")
        _ => imprimir!("otro")
"#,
        r#"struct Punto { x: i32, y: i32 }
impl Punto {
    fn cuadrado(&self) -> i32 { self.x * self.x + self.y * self.y }
}
fn main() {
    let punto = Punto { x: 3, y: 4 };
    match punto.cuadrado() { 25 => println!("25"), _ => println!("otro") }
}
"#,
    );
}

#[test]
fn differential_result_propagation_matches_native_rust() {
    assert_ox_and_rust_output_match(
        r#"sintaxis indentacion
funcion validar(divisor: i32) -> Resultado<i32, &'static str>:
    si divisor == 0:
        Err("division por cero")
    sino:
        Ok(12 / divisor)
funcion calcular() -> Resultado<i32, &'static str>:
    dejar valor = validar(3)?
    Ok(valor + 1)
funcion principal():
    coincidir calcular():
        Bien(valor) => imprimir!("resultado={}", valor)
        Falla(error) => imprimir!("error={}", error)
"#,
        r#"fn validar(divisor: i32) -> Result<i32, &'static str> {
    if divisor == 0 { Err("division por cero") } else { Ok(12 / divisor) }
}
fn calcular() -> Result<i32, &'static str> {
    let valor = validar(3)?;
    Ok(valor + 1)
}
fn main() {
    match calcular() {
        Ok(valor) => println!("resultado={}", valor),
        Err(error) => println!("error={}", error),
    }
}
"#,
    );
}

#[test]
fn differential_type_errors_match_rustc_and_map_back_to_the_ox_line() {
    use oxido_compiler::application::cargo::CargoAction;
    use oxido_language::domain::syntax::LexOptions;
    use std::process::Command;
    use std::time::{SystemTime, UNIX_EPOCH};

    let ox = SourceFile::new("sintaxis indentacion\nfuncion main():\n    dejar valor: bool = 1\n");
    let rust = "fn main() {\n    let valor: bool = 1;\n}\n";
    let result = oxido_compiler::build_ox(&ox, LexOptions::default(), CargoAction::Check)
        .unwrap_or_else(|diagnostics| panic!("{diagnostics:?}"));
    assert!(!result.success);
    let diagnostic = result
        .diagnostics
        .iter()
        .find(|diagnostic| diagnostic.code.as_deref() == Some("E0308"))
        .expect("Óxido conserva el código de error de tipos de rustc");
    let mapped = diagnostic
        .source
        .expect("el diagnóstico debe mapearse al archivo .ox");
    assert_eq!(mapped.line, 3);

    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock is after Unix epoch")
        .as_nanos();
    let directory = std::env::temp_dir().join(format!(
        "oxido-rust-negative-differential-{}-{nonce}",
        std::process::id()
    ));
    std::fs::create_dir(&directory).expect("create negative differential test directory");
    let source_path = directory.join("main.rs");
    std::fs::write(&source_path, rust).expect("write direct Rust negative fixture");
    let compile = Command::new("rustc")
        .arg("--edition=2024")
        .arg("--error-format=json")
        .arg("--emit=metadata")
        .arg("--out-dir")
        .arg(&directory)
        .arg(&source_path)
        .output()
        .expect("run rustc for direct negative fixture");
    assert!(!compile.status.success());
    let stderr = String::from_utf8(compile.stderr).expect("UTF-8 rustc diagnostics");
    let has_same_error = stderr.lines().any(|line| {
        serde_json::from_str::<serde_json::Value>(line)
            .ok()
            .and_then(|value| value.get("code").cloned())
            .and_then(|value| {
                value
                    .get("code")
                    .and_then(serde_json::Value::as_str)
                    .map(str::to_owned)
            })
            .as_deref()
            == Some("E0308")
    });
    assert!(has_same_error, "direct rustc diagnostics: {stderr}");
    std::fs::remove_dir_all(&directory).expect("remove negative differential test directory");
}

fn assert_ox_and_rust_output_match(ox_text: &str, rust: &str) {
    use oxido_compiler::application::cargo::CargoAction;
    use oxido_language::domain::syntax::LexOptions;
    use std::process::Command;
    use std::time::{SystemTime, UNIX_EPOCH};

    let ox = SourceFile::new(ox_text);
    let ox_result = oxido_compiler::build_ox(&ox, LexOptions::default(), CargoAction::Run)
        .unwrap_or_else(|diagnostics| panic!("{diagnostics:?}"));
    assert!(
        ox_result.success,
        "{} {:?}",
        ox_result.stderr, ox_result.diagnostics
    );

    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock is after Unix epoch")
        .as_nanos();
    let directory = std::env::temp_dir().join(format!(
        "oxido-rust-differential-{}-{nonce}",
        std::process::id()
    ));
    std::fs::create_dir(&directory).expect("create differential test directory");
    let source_path = directory.join("main.rs");
    let binary_path = directory.join("direct-rust");
    std::fs::write(&source_path, rust).expect("write direct Rust fixture");

    let compile = Command::new("rustc")
        .arg("--edition=2024")
        .arg(&source_path)
        .arg("-o")
        .arg(&binary_path)
        .output()
        .expect("run rustc for direct Rust fixture");
    assert!(
        compile.status.success(),
        "rustc failed: {}",
        String::from_utf8_lossy(&compile.stderr)
    );
    let direct_result = Command::new(&binary_path)
        .output()
        .expect("run direct Rust fixture");
    assert!(
        direct_result.status.success(),
        "direct Rust program failed: {}",
        String::from_utf8_lossy(&direct_result.stderr)
    );
    let direct_stdout = String::from_utf8(direct_result.stdout).expect("UTF-8 Rust stdout");
    assert_eq!(ox_result.program_output.join("\n") + "\n", direct_stdout);
    std::fs::remove_dir_all(&directory).expect("remove differential test directory");
}

#[test]
fn compiles_indented_declarative_macros_and_derive_attributes_as_rust() {
    use oxido_compiler::application::cargo::CargoAction;
    use oxido_language::domain::syntax::LexOptions;

    let source = SourceFile::new(
        "sintaxis indentacion\nmacro_rules! mostrar { ($valor:expr) => {{ let interno = $valor; println!(\"valor={}\", interno); }}; }\n#[derive(Debug)]\nestructura Dato:\n    valor: i32\nfuncion main():\n    dejar dato = Dato { valor: 7 };\n    mostrar!(dato.valor);\n",
    );
    let generated = oxido_compiler::generate_ox(&source, LexOptions::default())
        .unwrap_or_else(|diagnostics| panic!("{diagnostics:?}"));
    assert!(generated.rust.contains("macro_rules! mostrar"));
    assert!(generated.rust.contains("#[derive(Debug)]"));

    let result = oxido_compiler::build_ox(&source, LexOptions::default(), CargoAction::Run)
        .unwrap_or_else(|diagnostics| panic!("{diagnostics:?}"));
    assert!(result.success, "{} {:?}", result.stderr, result.diagnostics);
    assert_eq!(result.program_output, ["valor=7"]);
}

#[test]
fn validates_async_await_and_runs_standard_thread_concurrency() {
    use oxido_compiler::application::cargo::CargoAction;
    use oxido_language::domain::syntax::LexOptions;

    let source = SourceFile::new(
        "sintaxis indentacion\nasincrono funcion valor() -> i32:\n    retornar 20\nasincrono funcion respuesta() -> i32:\n    valor().esperar + 22\nfuncion main():\n    let mut future = Box::pin(respuesta());\n    let mut contexto = std::task::Context::from_waker(std::task::Waker::noop());\n    match std::future::Future::poll(future.as_mut(), &mut contexto) {\n        std::task::Poll::Ready(valor) => println!(\"async={}\", valor),\n        std::task::Poll::Pending => panic!(\"el fixture no debe quedar pendiente\"),\n    }\n    println!(\"thread={}\", std::thread::spawn(|| 42).join().unwrap());\n",
    );
    let generated = oxido_compiler::generate_ox(&source, LexOptions::default())
        .unwrap_or_else(|diagnostics| panic!("{diagnostics:?}"));
    assert!(generated.rust.contains("async fn valor()"));
    assert!(generated.rust.contains("valor().await"));

    let result = oxido_compiler::build_ox(&source, LexOptions::default(), CargoAction::Run)
        .unwrap_or_else(|diagnostics| panic!("{diagnostics:?}"));
    assert!(result.success, "{} {:?}", result.stderr, result.diagnostics);
    assert_eq!(result.program_output, ["async=42", "thread=42"]);
}

#[test]
fn preserves_valid_user_unsafe_api_without_blocking_by_default() {
    use oxido_compiler::application::cargo::CargoAction;
    use oxido_language::domain::syntax::LexOptions;

    let source = SourceFile::new(
        "sintaxis llaves\nfn main() { let valor = 42; let ptr = &valor as *const i32; unsafe { println!(\"{}\", *ptr); } }\n",
    );
    let result = oxido_compiler::build_ox(&source, LexOptions::default(), CargoAction::Run)
        .unwrap_or_else(|diagnostics| panic!("{diagnostics:?}"));
    assert!(result.success);
    assert_eq!(result.program_output, ["42"]);
}

#[test]
fn bilingual_block_modes_compile_to_the_same_behavior() {
    use oxido_compiler::application::cargo::CargoAction;
    use oxido_language::domain::syntax::LexOptions;

    let sources = [
        "sintaxis indentacion\nfuncion main():\n    si verdadero:\n        println!(\"equivalente\");\n    sino:\n        println!(\"distinto\");\n",
        "sintaxis llaves\nfuncion main() { si verdadero { println!(\"equivalente\"); } sino { println!(\"distinto\"); } }\n",
    ];
    for text in sources {
        let source = SourceFile::new(text);
        let generated = oxido_compiler::generate_ox(&source, LexOptions::default())
            .unwrap_or_else(|diagnostics| panic!("{diagnostics:?}"));
        assert!(generated.rust.contains("if true"), "{}", generated.rust);
        assert!(
            generated.rust.contains("println!(\"equivalente\")"),
            "{}",
            generated.rust
        );
        let result = oxido_compiler::build_ox(&source, LexOptions::default(), CargoAction::Run)
            .unwrap_or_else(|diagnostics| panic!("{diagnostics:?}"));
        assert!(result.success, "{} {:?}", result.stderr, result.diagnostics);
        assert_eq!(result.program_output, ["equivalente"]);
    }
}

#[test]
fn normalizes_aliases_in_visibility_signatures_and_passthrough_items() {
    use oxido_language::domain::syntax::LexOptions;

    let source = SourceFile::new(
        "sintaxis llaves\npublico usar std::fmt;\npublico funcion inspeccionar(valor: &dinamico std::fmt::Debug) { let _ = valor; }\n",
    );
    let generated = oxido_compiler::generate_ox(&source, LexOptions::default())
        .unwrap_or_else(|diagnostics| panic!("{diagnostics:?}"));
    assert!(
        generated.rust.contains("pub use std::fmt;"),
        "{}",
        generated.rust
    );
    assert!(
        generated
            .rust
            .contains("pub fn inspeccionar(valor: &dyn std::fmt::Debug)"),
        "{}",
        generated.rust
    );
}

#[test]
fn compiles_and_runs_indented_structs_enums_and_collections() {
    use oxido_compiler::application::cargo::CargoAction;
    use oxido_language::domain::syntax::LexOptions;

    let source = SourceFile::new(include_str!("../../../examples/fase_7_estructuras.ox"));
    let result = oxido_compiler::build_ox(&source, LexOptions::default(), CargoAction::Run)
        .unwrap_or_else(|diagnostics| panic!("{diagnostics:?}"));
    assert!(result.success, "{} {:?}", result.stderr, result.diagnostics);
    assert_eq!(result.program_output, ["5:1"]);
}

#[test]
fn compiles_and_runs_spanish_prelude_aliases_and_macros() {
    use oxido_compiler::application::cargo::CargoAction;
    use oxido_language::domain::syntax::LexOptions;

    let source = SourceFile::new(include_str!("../../../examples/fase_7_prelude_espanol.ox"));
    let generated = oxido_compiler::generate_ox(&source, LexOptions::default())
        .unwrap_or_else(|diagnostics| panic!("{diagnostics:?}"));
    for rust in [
        "fn main()",
        "Vec<String>",
        "vec![",
        ".push(",
        ".len(",
        "println!(",
    ] {
        assert!(
            generated.rust.contains(rust),
            "missing {rust}: {}",
            generated.rust
        );
    }
    let result = oxido_compiler::build_ox(&source, LexOptions::default(), CargoAction::Run)
        .unwrap_or_else(|diagnostics| panic!("{diagnostics:?}"));
    assert!(result.success, "{} {:?}", result.stderr, result.diagnostics);
    assert_eq!(result.program_output, ["Ada:4"]);
}

#[test]
fn compiles_all_published_standard_macro_aliases_with_cargo_and_rustc() {
    use oxido_compiler::application::cargo::CargoAction;
    use oxido_language::domain::syntax::LexOptions;

    let source = SourceFile::new(
        r#"sintaxis llaves
use std::fmt::Write;
local_del_hilo! { static CONTADOR: std::cell::Cell<u32> = const { std::cell::Cell::new(0) }; }
fn main() {
    imprimir!("A");
    imprimir_sin_salto!("B");
    imprimir!("C");
    imprimir_error!("error de prueba");
    imprimir_error_sin_salto!("!");
    let cadena = formato!("{}", 7);
    let cadena_dos = formato!("{}!", cadena);
    let argumentos = argumentos_formato!("{}:{}", cadena, 8);
    let mut buffer = String::new();
    escribir!(&mut buffer, "{}", argumentos).expect("write");
    escribir_linea!(&mut buffer, "fin").expect("writeln");
    let valores = vector![1, 2, 3];
    afirmar!(valores.len() == 3);
    afirmar_igual!(valores[0], 1);
    afirmar_distinto!(valores[0], valores[1]);
    afirmar_debug!(verdadero);
    afirmar_igual_debug!(valores.len(), 3);
    afirmar_distinto_debug!(valores[0], valores[1]);
    depurar!(valores.len());
    afirmar!(coincide!(valores.first(), Some(&1)));
    afirmar!(incluir_texto!("main.rs").contains("fn main"));
    afirmar!(!incluir_bytes!("main.rs").is_empty());
    afirmar!(!entorno!("CARGO_PKG_NAME").is_empty());
    let version = entorno_opcional!("CARGO_PKG_VERSION");
    afirmar!(version.is_some());
    afirmar_igual!(concatenar!("o", "x"), "ox");
    afirmar_igual!(cadena_dos, "7!");
    afirmar_igual!(a_texto!(tipo), "tipo");
    afirmar!(linea!() > 0);
    afirmar!(!archivo!().is_empty());
    afirmar!(columna!() > 0);
    afirmar!(config!(debug_assertions));
    CONTADOR.with(|contador| contador.set(1));
    afirmar_igual!(CONTADOR.with(std::cell::Cell::get), 1);
    afirmar_igual!(format!("{}", buffer), "7:8fin\n");
    afirmar_igual!(vec![4, 5].len(), 2);
    assert_eq!(format!("{}", 9), "9");
    if std::hint::black_box(false) {
        panico!("no se ejecuta");
        pendiente!();
        sin_implementar!();
        inalcanzable!();
    }
}
"#,
    );
    let generated = oxido_compiler::generate_ox(&source, LexOptions::default())
        .unwrap_or_else(|diagnostics| panic!("{diagnostics:?}"));
    for (alias, rust_name) in [
        ("imprimir!", "println!"),
        ("imprimir_sin_salto!", "print!"),
        ("imprimir_error!", "eprintln!"),
        ("imprimir_error_sin_salto!", "eprint!"),
        ("formato!", "format!"),
        ("argumentos_formato!", "format_args!"),
        ("escribir!", "write!"),
        ("escribir_linea!", "writeln!"),
        ("vector!", "vec!"),
        ("panico!", "panic!"),
        ("afirmar!", "assert!"),
        ("afirmar_igual!", "assert_eq!"),
        ("afirmar_distinto!", "assert_ne!"),
        ("afirmar_debug!", "debug_assert!"),
        ("afirmar_igual_debug!", "debug_assert_eq!"),
        ("afirmar_distinto_debug!", "debug_assert_ne!"),
        ("depurar!", "dbg!"),
        ("pendiente!", "todo!"),
        ("sin_implementar!", "unimplemented!"),
        ("inalcanzable!", "unreachable!"),
        ("coincide!", "matches!"),
        ("incluir_texto!", "include_str!"),
        ("incluir_bytes!", "include_bytes!"),
        ("entorno!", "env!"),
        ("entorno_opcional!", "option_env!"),
        ("concatenar!", "concat!"),
        ("a_texto!", "stringify!"),
        ("linea!", "line!"),
        ("archivo!", "file!"),
        ("columna!", "column!"),
        ("config!", "cfg!"),
        ("local_del_hilo!", "thread_local!"),
    ] {
        assert!(
            generated.rust.contains(rust_name),
            "falta {alias}: {}",
            generated.rust
        );
    }

    let result = oxido_compiler::build_ox(&source, LexOptions::default(), CargoAction::Run)
        .unwrap_or_else(|diagnostics| panic!("{diagnostics:?}"));
    assert!(result.success, "{} {:?}", result.stderr, result.diagnostics);
    assert_eq!(result.program_output, ["A", "BC"]);

    let compile_error = SourceFile::new(
        "sintaxis llaves\nfn main() { error_de_compilacion!(\"fallo esperado\"); }\n",
    );
    let rejected =
        oxido_compiler::build_ox(&compile_error, LexOptions::default(), CargoAction::Check)
            .unwrap_or_else(|diagnostics| panic!("{diagnostics:?}"));
    assert!(!rejected.success);
    let generated_error = oxido_compiler::generate_ox(&compile_error, LexOptions::default())
        .unwrap_or_else(|diagnostics| panic!("{diagnostics:?}"));
    assert!(generated_error.rust.contains("compile_error!"));
}

#[test]
fn compiles_and_runs_indented_impl_traits_modules_and_match_arms() {
    use oxido_compiler::application::cargo::CargoAction;
    use oxido_language::domain::syntax::LexOptions;

    let source = SourceFile::new(include_str!(
        "../../../examples/fase_8_estructura_indentada.ox"
    ));
    let generated = oxido_compiler::generate_ox(&source, LexOptions::default())
        .unwrap_or_else(|diagnostics| panic!("{diagnostics:?}"));
    for rust in [
        "impl Contador",
        "trait Describible",
        "mod utilidades",
        "mod operaciones",
        "match contador.leer()",
    ] {
        assert!(
            generated.rust.contains(rust),
            "missing {rust}: {}",
            generated.rust
        );
    }
    let result = oxido_compiler::build_ox(&source, LexOptions::default(), CargoAction::Run)
        .unwrap_or_else(|diagnostics| panic!("{diagnostics:?}"));
    assert!(result.success, "{} {:?}", result.stderr, result.diagnostics);
    assert_eq!(result.program_output, ["valor 4: cuatro"]);
}

#[test]
fn compiles_generics_lifetimes_closures_iterators_and_patterns_in_both_modes() {
    use oxido_compiler::application::cargo::CargoAction;
    use oxido_language::domain::syntax::LexOptions;

    let sources = [
        SourceFile::new(
            r#"sintaxis indentacion
funcion seleccionar<'a, T>(items: &'a [T], aceptar: impl Fn(&T) -> bool) -> Option<&'a T> donde T: 'a:
    items.iter().find(|item| aceptar(item))

funcion principal():
    dejar nombres = ["Ana", "Luis", "Ada"]
    dejar elegido = seleccionar(&nombres, |nombre| nombre.starts_with('A'))
    coincidir elegido:
        Some(nombre) => imprimir!("{}", nombre)
        None => imprimir!("ninguno")
"#,
        ),
        SourceFile::new(
            r#"sintaxis llaves
funcion seleccionar<'a, T>(items: &'a [T], aceptar: impl Fn(&T) -> bool) -> Option<&'a T> donde T: 'a { items.iter().find(|item| aceptar(item)) }
funcion principal() {
    let nombres = ["Ana", "Luis", "Ada"];
    let elegido = seleccionar(&nombres, |nombre| nombre.starts_with('A'));
    match elegido { Some(nombre) => println!("{}", nombre), None => println!("ninguno") }
}
"#,
        ),
    ];

    let mut outputs = Vec::new();
    for source in sources {
        let generated = oxido_compiler::generate_ox(&source, LexOptions::default())
            .unwrap_or_else(|diagnostics| panic!("{diagnostics:?}"));
        assert!(generated.rust.contains("<'a, T>"), "{}", generated.rust);
        assert!(generated.rust.contains("impl Fn"), "{}", generated.rust);
        assert!(generated.rust.contains(".find("), "{}", generated.rust);

        let result = oxido_compiler::build_ox(&source, LexOptions::default(), CargoAction::Run)
            .unwrap_or_else(|diagnostics| panic!("{diagnostics:?}"));
        assert!(result.success, "{} {:?}", result.stderr, result.diagnostics);
        outputs.push(result.program_output);
    }

    assert_eq!(outputs, [vec!["Ana".to_owned()], vec!["Ana".to_owned()]]);
}

#[test]
fn impl_methods_generics_associated_items_tail_values_and_block_modes_agree() {
    use oxido_compiler::application::cargo::CargoAction;
    use oxido_language::domain::syntax::LexOptions;

    let indentation = SourceFile::new(
        r#"sintaxis indentacion

#[derivar(Clone)]
estructura Dato<T>:
    valor: T

rasgo Contenido:
    tipo Elemento;
    funcion elemento(&mismo) -> &Mismo::Elemento;

implementar<T> Dato<T> donde T: Clone:
    publico constante LIMITE: usize = 2;
    publico funcion nuevo(valor: T) -> Mismo:
        Mismo { valor }
    /// Copia el dato para el llamador.
    #[inline]
    publico funcion clonar(&mismo) -> Mismo:
        Mismo { valor: mismo.valor.clone() }
    publico funcion avanzar(&mutable mismo, valor: T):
        mismo.valor = valor

implementar<T> Contenido para Dato<T> donde T: Clone:
    tipo Elemento = T;
    funcion elemento(&mismo) -> &Mismo::Elemento:
        &mismo.valor

implementar Dato<i32>:
    funcion limite() -> usize:
        Dato::<i32>::LIMITE
    funcion limite_unit():
        7;
    funcion consumir(mismo) -> i32:
        mismo.valor

funcion principal():
    dejar mutable dato = Dato::nuevo(4)
    dato.avanzar(5)
    dejar prestado = dato.elemento()
    dejar duplicado = dato.clonar()
    dejar limite: usize = Dato::<i32>::limite()
    dejar unidad: () = Dato::<i32>::limite_unit()
    dejar consumido = duplicado.consumir()
    imprimir!("{}:{}:{}:{}", dato.valor, *prestado, limite, consumido)
"#,
    );
    let generated = oxido_compiler::generate_ox(&indentation, LexOptions::default())
        .unwrap_or_else(|diagnostics| panic!("{diagnostics:?}"));
    assert!(
        generated.rust.contains("impl<T> Dato<T>"),
        "{}",
        generated.rust
    );
    assert!(generated.rust.contains("T: Clone"), "{}", generated.rust);
    assert!(
        generated.rust.contains("pub const LIMITE: usize = 2;"),
        "{}",
        generated.rust
    );
    assert!(generated.rust.contains("#[inline]"), "{}", generated.rust);
    assert!(
        generated.rust.contains("Copia el dato para el llamador"),
        "{}",
        generated.rust
    );
    let indented_result =
        oxido_compiler::build_ox(&indentation, LexOptions::default(), CargoAction::Run)
            .unwrap_or_else(|diagnostics| panic!("{diagnostics:?}"));
    assert!(
        indented_result.success,
        "{} {:?}",
        indented_result.stderr, indented_result.diagnostics
    );
    assert_eq!(indented_result.program_output, ["5:5:2:5"]);

    let braces = SourceFile::new(
        r#"sintaxis llaves
#[derive(Clone)] struct Dato<T> { valor: T }
trait Contenido { type Elemento; fn elemento(&self) -> &Self::Elemento; }
impl<T> Dato<T> where T: Clone {
    pub const LIMITE: usize = 2;
    pub fn nuevo(valor: T) -> Self { Self { valor } }
    pub fn clonar(&self) -> Self { Self { valor: self.valor.clone() } }
    pub fn avanzar(&mut self, valor: T) { self.valor = valor; }
}
impl<T> Contenido for Dato<T> where T: Clone {
    type Elemento = T;
    fn elemento(&self) -> &Self::Elemento { &self.valor }
}
impl Dato<i32> {
    fn limite() -> usize { Dato::<i32>::LIMITE }
    fn limite_unit() { 7; }
    fn consumir(self) -> i32 { self.valor }
}
funcion principal() {
    dejar mutable dato = Dato::nuevo(4);
    dato.avanzar(5);
    dejar prestado = dato.elemento();
    dejar duplicado = dato.clonar();
    dejar limite: usize = Dato::<i32>::limite();
    dejar unidad: () = Dato::<i32>::limite_unit();
    dejar consumido = duplicado.consumir();
    println!("{}:{}:{}:{}", dato.valor, *prestado, limite, consumido);
}
"#,
    );
    let braced_result = oxido_compiler::build_ox(&braces, LexOptions::default(), CargoAction::Run)
        .unwrap_or_else(|diagnostics| panic!("{diagnostics:?}"));
    assert!(
        braced_result.success,
        "{} {:?}",
        braced_result.stderr, braced_result.diagnostics
    );
    assert_eq!(braced_result.program_output, indented_result.program_output);
}

#[test]
fn rejects_empty_impl_and_reports_invalid_method_indentation() {
    use oxido_language::domain::syntax::LexOptions;

    let empty = SourceFile::new(
        "sintaxis indentacion\nestructura Dato:\n    valor: i32\n\nimplementar Dato:\n",
    );
    let parsed = oxido_compiler::parse_ox(&empty, LexOptions::default());
    assert!(parsed.has_errors());
    assert!(parsed.diagnostics.iter().any(|diagnostic| {
        diagnostic.code == "OX2003" && diagnostic.message.contains("no puede estar vacío")
    }));

    let malformed = SourceFile::new(
        "sintaxis indentacion\nimplementar Dato:\n    funcion leer(mismo) -> i32:\n        mismo.valor\n      funcion otro(mismo) -> i32:\n          0\n",
    );
    let parsed = oxido_compiler::parse_ox(&malformed, LexOptions::default());
    assert!(parsed.has_errors());
    assert!(
        parsed
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "OX2001")
    );
}

#[test]
fn maps_rustc_errors_inside_indented_methods_to_the_method_line() {
    use oxido_compiler::application::cargo::CargoAction;
    use oxido_language::domain::syntax::LexOptions;

    let source = SourceFile::new(
        "sintaxis indentacion\nestructura Dato:\n    valor: i32\nimplementar Dato:\n    funcion leer(&mismo) -> i32:\n        nombre_que_no_existe\nfuncion principal():\n    dejar dato = Dato { valor: 1 }\n    dato.leer()\n",
    );
    let result = oxido_compiler::build_ox(&source, LexOptions::default(), CargoAction::Check)
        .unwrap_or_else(|diagnostics| panic!("{diagnostics:?}"));
    let diagnostic = result
        .diagnostics
        .iter()
        .find(|diagnostic| diagnostic.code.as_deref() == Some("E0425"))
        .expect("el error del método debe conservar su código rustc");
    let location = diagnostic.source.expect("el span debe mapear al .ox");
    assert_eq!(location.line, 6);
    let span = source.span(location.start, location.end).unwrap();
    assert!(source.slice(span).contains("nombre_que_no_existe"));
}

#[test]
fn preserves_outer_and_inner_documentation_comments_at_indented_block_boundaries() {
    use oxido_language::domain::syntax::LexOptions;

    let source = SourceFile::new(
        r#"sintaxis indentacion
#![forbid(unsafe_code)]
//! Documentación del crate.
/*! Segunda línea de documentación del crate. */
estructura Punto:
    x: i32
implementar Punto: // comentario al abrir el bloque
    /// Documentación del primer método.

    #[inline]
    funcion primero(&mismo) -> i32:
        mismo.x
    /**
     * Documentación del método intermedio.
     */
    funcion segundo(&mismo) -> i32:
        mismo.x
    /// Documentación del último método.
    funcion ultimo(&mismo) -> i32:
        mismo.x
    // Comentario ordinario al final del bloque.
funcion principal():
    dejar punto = Punto { x: 1 }
    imprimir!("{}", punto.ultimo())
"#,
    );
    let generated = oxido_compiler::generate_ox(&source, LexOptions::default())
        .unwrap_or_else(|diagnostics| panic!("{diagnostics:?}"));

    for documentation in [
        "#![forbid(unsafe_code)]",
        "//!Documentación del crate.",
        "//!Segunda línea de documentación del crate.",
        "Documentación del primer método.",
        "Documentación del método intermedio.",
        "Documentación del último método.",
    ] {
        assert!(
            generated.rust.contains(documentation),
            "falta {documentation}: {}",
            generated.rust
        );
    }
    assert!(
        generated
            .rust
            .contains("// Comentario ordinario al final del bloque.")
    );
    assert!(generated.rust.contains("// comentario al abrir el bloque"));
}

#[test]
fn preserves_ordinary_comments_at_structural_boundaries_when_exporting() {
    use oxido_language::domain::syntax::LexOptions;

    let source = SourceFile::new(
        r#"sintaxis indentacion
// antes del item

// también antes del item, tras una línea vacía
funcion principal():
    dejar valor = 1 // al final de una sentencia
    si valor == 1:
        imprimir!("uno ñ")
        // al final del bloque si
    /* bloque multilínea
     * entre instrucciones
     */
    coincidir valor:
        1 => imprimir!("coincide")
        // entre brazos
        _ => imprimir!("otro")
// al final del archivo
"#,
    );
    let generated = oxido_compiler::generate_ox(&source, LexOptions::default())
        .unwrap_or_else(|diagnostics| panic!("{diagnostics:?}"));

    for comment in [
        "// antes del item",
        "// también antes del item, tras una línea vacía",
        "// al final de una sentencia",
        "// al final del bloque si",
        "/* bloque multilínea\n     * entre instrucciones\n     */",
        "// entre brazos",
        "// al final del archivo",
    ] {
        assert!(
            generated.rust.contains(comment),
            "falta {comment:?}: {}",
            generated.rust
        );
    }
    assert!(
        generated.rust.find("// antes del item").unwrap() < generated.rust.find("fn main").unwrap()
    );
    assert!(
        generated
            .rust
            .contains("// antes del item\n\n// también antes del item, tras una línea vacía")
    );
    assert!(generated.rust.find("// entre brazos").unwrap() < generated.rust.find("_ =>").unwrap());
    assert!(
        generated
            .rust
            .contains("println!(\"uno ñ\")\n        // al final del bloque si\n    }")
    );
    assert!(
        generated
            .rust
            .trim_end()
            .ends_with("// al final del archivo"),
        "{}",
        generated.rust
    );

    let result = oxido_compiler::build_ox(
        &source,
        LexOptions::default(),
        oxido_compiler::application::cargo::CargoAction::Run,
    )
    .unwrap_or_else(|diagnostics| panic!("{diagnostics:?}"));
    assert!(result.success, "{} {:?}", result.stderr, result.diagnostics);
    assert_eq!(result.program_output, ["uno ñ", "coincide"]);

    let macro_comment = SourceFile::new(
        "sintaxis llaves\nfn main() { println!(\"{}\", /* comentario opaco */ 1); }\n",
    );
    let error = oxido_compiler::generate_ox(&macro_comment, LexOptions::default())
        .expect_err("no debe descartar silenciosamente comentarios dentro de macros");
    assert!(error.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("grupo opaco de macro o atributo")
    }));

    for source in [
        "sintaxis llaves\nfn main() { crate_externo::dsl!({ /* comentario interno */ nombre: [tipo, verdadero] }); }\n",
        "sintaxis llaves\n#[crate_externo::atributo(/* comentario interno */ tipo)]\nfn main() {}\n",
    ] {
        let source = SourceFile::new(source);
        let diagnostics = oxido_compiler::generate_ox(&source, LexOptions::default())
            .expect_err("los comentarios en grupos opacos no deben perderse silenciosamente");
        assert!(diagnostics.iter().any(|diagnostic| {
            diagnostic.code == "OX4001"
                && diagnostic
                    .message
                    .contains("grupo opaco de macro o atributo")
        }));
    }
}

#[test]
fn rejects_indented_match_arms_without_a_pattern_separator() {
    use oxido_language::domain::syntax::LexOptions;

    let source = SourceFile::new(
        "sintaxis indentacion\nfuncion principal():\n    coincidir 1:\n        1 \"uno\"\n",
    );
    let parsed = oxido_compiler::parse_ox(&source, LexOptions::default());
    assert!(parsed.has_errors());
    assert!(
        parsed
            .diagnostics
            .iter()
            .any(|diagnostic| { diagnostic.code == "OX2003" && diagnostic.message.contains("=>") })
    );
}

#[test]
fn compiles_allowlisted_spanish_attributes_on_rust_items() {
    use oxido_compiler::application::cargo::CargoAction;
    use oxido_language::domain::syntax::LexOptions;

    let source = SourceFile::new(
        "sintaxis llaves\n#[derivar(Clone, Defecto)] struct Dato;\n#[permitir(dead_code)] fn auxiliar() {}\n#[prueba] fn prueba_alias() { let _ = Dato; }\nfuncion principal() { let _ = Dato; }\n",
    );
    let generated = oxido_compiler::generate_ox(&source, LexOptions::default())
        .unwrap_or_else(|diagnostics| panic!("{diagnostics:?}"));
    assert!(
        generated.rust.contains("#[derive(Clone, Default)]"),
        "{}",
        generated.rust
    );
    assert!(
        generated.rust.contains("#[allow(dead_code)]"),
        "{}",
        generated.rust
    );
    assert!(generated.rust.contains("#[test]"), "{}", generated.rust);
    let checked = oxido_compiler::build_ox(&source, LexOptions::default(), CargoAction::Check)
        .unwrap_or_else(|diagnostics| panic!("{diagnostics:?}"));
    assert!(
        checked.success,
        "{} {:?}",
        checked.stderr, checked.diagnostics
    );
}

#[test]
fn rejects_malformed_indented_struct_fields_with_a_rust_syntax_diagnostic() {
    use oxido_language::domain::syntax::LexOptions;

    let source = SourceFile::new("sintaxis indentacion\nestructura Punto:\n    x i32\n");
    let parsed = oxido_compiler::parse_ox(&source, LexOptions::default());
    assert!(parsed.has_errors());
    assert!(parsed.diagnostics.iter().any(|diagnostic| {
        diagnostic.code == "OX4001"
            && diagnostic
                .message
                .contains("item estructural Rust inválido")
    }));
}

#[test]
fn maps_a_rust_type_error_in_an_indented_struct_field_back_to_ox() {
    use oxido_compiler::application::cargo::CargoAction;
    use oxido_language::domain::syntax::LexOptions;

    let source = SourceFile::new(
        "sintaxis indentacion\nestructura Punto:\n    x: TipoInexistente\nfuncion main():\n    dejar _punto = Punto { x: 1 }\n",
    );
    let result = oxido_compiler::build_ox(&source, LexOptions::default(), CargoAction::Check)
        .unwrap_or_else(|diagnostics| panic!("{diagnostics:?}"));
    assert!(!result.success);
    let diagnostic = result
        .diagnostics
        .iter()
        .find(|diagnostic| diagnostic.code.as_deref() == Some("E0425"))
        .unwrap_or_else(|| panic!("diagnóstico no esperado: {:?}", result.diagnostics));
    let mapped = diagnostic
        .source
        .expect("el campo generado debe mapear a su ubicación .ox");
    let span = source.span(mapped.start, mapped.end).unwrap();
    assert!(source.slice(span).contains("TipoInexistente"));
}

#[test]
fn runs_initial_rust_interop_features_through_the_braces_mode() {
    use oxido_compiler::application::cargo::CargoAction;
    use oxido_language::domain::syntax::LexOptions;

    let source = SourceFile::new(include_str!(
        "../../../examples/fase_7_interoperabilidad.ox"
    ));
    let result = oxido_compiler::build_ox(&source, LexOptions::default(), CargoAction::Run)
        .unwrap_or_else(|diagnostics| panic!("{diagnostics:?}"));
    assert!(result.success, "{} {:?}", result.stderr, result.diagnostics);
    assert_eq!(result.program_output, ["5:1:3:5"]);
}

#[test]
fn maps_rustc_type_errors_back_to_the_ox_expression() {
    use oxido_compiler::application::cargo::CargoAction;
    use oxido_language::domain::syntax::LexOptions;

    let source = SourceFile::new(
        "sintaxis indentacion\n// comentario antes de función\nfuncion main():\n    // comentario antes del error\n    dejar valor: bool = 1\n",
    );
    let result = oxido_compiler::build_ox(&source, LexOptions::default(), CargoAction::Check)
        .unwrap_or_else(|diagnostics| panic!("{diagnostics:?}"));
    assert!(!result.success);
    let diagnostic = result
        .diagnostics
        .iter()
        .find(|diagnostic| diagnostic.code.as_deref() == Some("E0308"))
        .expect("se conserva el diagnostic JSON del compilador Rust");
    let mapped = diagnostic.source.expect("el span debe volver al .ox");
    assert_eq!(mapped.line, 5);
    let span = source.span(mapped.start, mapped.end).unwrap();
    assert!(source.slice(span).contains('1'));
}

#[test]
fn maps_rustc_control_header_errors_back_to_the_ox_header() {
    use oxido_compiler::application::cargo::CargoAction;
    use oxido_language::domain::syntax::LexOptions;

    let source = SourceFile::new("sintaxis llaves\nfn main() { if 1 { println!(\"nunca\"); } }\n");
    let result = oxido_compiler::build_ox(&source, LexOptions::default(), CargoAction::Check)
        .unwrap_or_else(|diagnostics| panic!("{diagnostics:?}"));
    assert!(!result.success);
    let diagnostic = result
        .diagnostics
        .iter()
        .find(|diagnostic| diagnostic.code.as_deref() == Some("E0308"))
        .expect("se conserva el error de condición Rust");
    let mapped = diagnostic
        .source
        .expect("el span de condición debe mapearse");
    let span = source.span(mapped.start, mapped.end).unwrap();
    assert_eq!(source.slice(span), "1");
}

#[test]
fn imports_rust_passthrough_items_during_tree_import() {
    let source = SourceFile::new("struct Dato { valor: i32 }\n");
    let imported = oxido_compiler::import_rust(&source).unwrap();
    assert!(imported.contains("struct Dato"), "{imported}");
}

#[test]
fn returns_a_parser_error_for_invalid_rust() {
    let source = SourceFile::new("fn { ");
    assert!(oxido_compiler::analyze_rust(&source).is_err());
}

#[test]
fn parses_ox_structure_and_validates_rust_fragments_with_syn() {
    use oxido_language::domain::syntax::LexOptions;

    let source =
        SourceFile::new("sintaxis indentacion\nfuncion sumar(a: i32, b: i32) -> i32:\n    a + b\n");
    let parsed = oxido_compiler::parse_ox(&source, LexOptions::default());
    assert!(!parsed.has_errors(), "{:?}", parsed.diagnostics);
    assert_eq!(parsed.file.unwrap().items.len(), 1);

    let continued =
        SourceFile::new("sintaxis indentacion\nfuncion rota():\n    dejar x = 1 +\n    x\n");
    let parsed = oxido_compiler::parse_ox(&continued, LexOptions::default());
    assert!(!parsed.has_errors(), "{:?}", parsed.diagnostics);

    let malformed = SourceFile::new("sintaxis indentacion\nfuncion rota():\n    dejar x = +\n");
    let parsed = oxido_compiler::parse_ox(&malformed, LexOptions::default());
    assert!(parsed.has_errors());
    assert!(parsed.file.is_none());
    assert!(
        parsed
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "OX4001")
    );
}

#[test]
fn validates_passthrough_items_and_all_initial_control_statements_with_syn() {
    use oxido_language::domain::syntax::LexOptions;

    let source = SourceFile::new(
        "sintaxis llaves\n#[derive(Clone)]\npub struct Dato { valor: i32 }\npub(crate) use std::fmt;\ntest_support::assertion!();\n#[inline]\npub async fn cargar() {}\nfn recorrer(xs: &[i32]) { for x in xs { while *x > 0 { if *x == 1 { break; } else { continue; } } } loop { break; } }\n",
    );
    let parsed = oxido_compiler::parse_ox(&source, LexOptions::default());
    assert!(!parsed.has_errors(), "{:?}", parsed.diagnostics);
    assert_eq!(parsed.file.unwrap().items.len(), 5);
}

#[test]
fn validates_each_supported_top_level_item_kind_in_braces_mode() {
    use oxido_language::domain::syntax::LexOptions;

    let source = SourceFile::new(
        "sintaxis llaves\n#[repr(C)] pub struct Dato { valor: i32 }\nenum Color { Rojo, Azul }\nimpl Dato { fn nuevo() -> Self { Self { valor: 0 } } }\ntrait Leer { fn leer(&self) -> i32; }\nmod interno { pub fn valor() -> i32 { 1 } }\nuse std::fmt;\ntype Alias = i32;\nconst LIMITE: i32 = 10;\nstatic GLOBAL: i32 = 0;\nunion Crudo { valor: i32 }\nmacro_rules! ejemplo { () => {} }\nextern \"C\" { fn llamada_externa(); }\n",
    );
    let parsed = oxido_compiler::parse_ox(&source, LexOptions::default());
    assert!(!parsed.has_errors(), "{:?}", parsed.diagnostics);
    assert_eq!(parsed.file.unwrap().items.len(), 12);
}

#[test]
fn reports_malformed_function_attributes() {
    use oxido_language::domain::syntax::LexOptions;

    let source = SourceFile::new("sintaxis llaves\n#[doc = ]\nfn invalida() {}\n");
    let parsed = oxido_compiler::parse_ox(&source, LexOptions::default());
    assert!(parsed.has_errors());
    assert!(
        parsed
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "OX4001")
    );
}

#[test]
fn validates_the_same_control_subset_in_both_block_modes() {
    use oxido_language::domain::syntax::LexOptions;

    let sources = [
        "sintaxis indentacion\nfn recorrer(xs: &[i32]) -> i32:\n    for x in xs:\n        while *x > 0:\n            if *x == 1:\n                break\n            else:\n                continue\n    loop:\n        return 0\n",
        "sintaxis llaves\nfn recorrer(xs: &[i32]) -> i32 { for x in xs { while *x > 0 { if *x == 1 { break; } else { continue; } } } loop { return 0; } }\n",
    ];
    for source in sources {
        let parsed = oxido_compiler::parse_ox(&SourceFile::new(source), LexOptions::default());
        assert!(!parsed.has_errors(), "{:?}", parsed.diagnostics);
        assert_eq!(parsed.file.unwrap().items.len(), 1);
    }
}

#[test]
fn reports_invalid_for_and_jump_fragments() {
    use oxido_language::domain::syntax::LexOptions;

    let source = SourceFile::new("sintaxis llaves\nfn invalida() { for x xs { break 1 2; } }\n");
    let parsed = oxido_compiler::parse_ox(&source, LexOptions::default());
    assert!(parsed.has_errors());
    assert!(parsed.file.is_none());
    assert_eq!(
        parsed
            .diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.code == "OX4001")
            .count(),
        2
    );
}

#[test]
fn validates_function_signatures_patterns_types_and_expressions() {
    use oxido_language::domain::syntax::LexOptions;

    for source in [
        "sintaxis llaves\nfn rota(x:) {}\n",
        "sintaxis llaves\nfn rota() { let x @ = 1; }\n",
        "sintaxis llaves\nfn rota() { let x: Vec< = Vec::new(); }\n",
        "sintaxis llaves\nfn rota() { let x = 1 +; }\n",
    ] {
        let parsed = oxido_compiler::parse_ox(&SourceFile::new(source), LexOptions::default());
        assert!(parsed.has_errors(), "{source}: {:?}", parsed.diagnostics);
        assert!(parsed.file.is_none(), "{source}");
    }
}

#[test]
fn generic_struct_field_types_do_not_join_the_next_indented_field() {
    use oxido_language::domain::syntax::LexOptions;
    let source = SourceFile::new(
        "sintaxis indentacion\nestructura Almacen:\n    productos: Vector<Producto>\n    siguiente_id: u32\ntipo Estado = Arc<Mutex<Almacen>>;\n",
    );
    let parsed = oxido_compiler::parse_ox(&source, LexOptions::default());
    assert!(!parsed.has_errors(), "{:?}", parsed.diagnostics);
    let generated = oxido_compiler::generate_ox(&source, LexOptions::default())
        .expect("los campos genéricos deben generar una estructura Rust válida");
    assert!(generated.rust.contains("productos"));
    assert!(generated.rust.contains("siguiente_id"));
}

#[test]
fn generates_the_indented_crud_example_as_valid_rust() {
    use oxido_language::domain::syntax::LexOptions;

    let source = SourceFile::new(include_str!("fixtures/crud_indented.ox"));
    let generated = oxido_compiler::generate_ox(&source, LexOptions::default())
        .unwrap_or_else(|diagnostics| panic!("{diagnostics:?}"));
    assert!(
        !generated
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "OX3002"),
        "una función libre `obtener` sin llamadas de método no debe producir OX3002: {:?}",
        generated.diagnostics
    );
    oxido_compiler::analyze_rust(&SourceFile::new(&generated.rust))
        .expect("el CRUD debe generar un archivo Rust sintácticamente válido");
    assert!(generated.rust.contains("async fn listar"));
    assert!(generated.rust.contains("struct Almacen"));
}

#[test]
fn prelude_aliases_collide_as_identifiers_with_raw_escape_guidance() {
    use oxido_language::domain::syntax::LexOptions;

    for (alias, _) in oxido_language::domain::syntax::PRELUDE_ALIASES {
        let source = SourceFile::new(format!(
            "sintaxis indentacion\nfuncion principal():\n    dejar {alias} = 1\n"
        ));
        let parsed = oxido_compiler::parse_ox(&source, LexOptions::default());
        assert!(
            parsed
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.code == "OX3001" && diagnostic.message.contains("r#")),
            "el alias {alias} debe sugerir escape como identificador"
        );
        let raw_source = SourceFile::new(format!(
            "sintaxis indentacion\nfuncion principal():\n    dejar r#{alias} = 1\n"
        ));
        let raw = oxido_compiler::parse_ox(&raw_source, LexOptions::default());
        assert!(
            !raw.has_errors(),
            "escape raw {alias}: {:?}",
            raw.diagnostics
        );
    }
}

#[test]
fn compiles_and_runs_the_indented_student_example_with_static_inference() {
    use oxido_compiler::application::cargo::CargoAction;
    use oxido_language::domain::syntax::LexOptions;

    let source = SourceFile::new(include_str!("fixtures/alumnos_inferencia.ox"));
    let result = oxido_compiler::build_ox(&source, LexOptions::default(), CargoAction::Run)
        .unwrap_or_else(|diagnostics| panic!("{diagnostics:?}"));
    assert!(result.success, "{} {:?}", result.stderr, result.diagnostics);
    assert_eq!(
        result.program_output,
        ["Ana aprobó", "Luis reprobó", "Promedio: 6.75"]
    );
}

#[test]
fn declared_spanish_method_names_are_not_rewritten_as_standard_aliases() {
    use oxido_compiler::application::cargo::CargoAction;
    use oxido_language::domain::{diagnostics::Severity, syntax::LexOptions};

    let source = SourceFile::new(
        "sintaxis indentacion\nestructura Repositorio:\n    valor: i32\nimplementar Repositorio:\n    funcion obtener(&mismo) -> i32:\n        mismo.valor\nfuncion principal():\n    dejar repo = Repositorio { valor: 7 }\n    dejar valores = Vector::from([1])\n    dejar _primero = valores.get(0)\n    imprimir!(\"{}\", repo.obtener())\n",
    );
    let parsed = oxido_compiler::parse_ox(&source, LexOptions::default());
    assert!(
        parsed.diagnostics.iter().any(|diagnostic| {
            diagnostic.code == "OX3002"
                && diagnostic.severity == Severity::Warning
                && diagnostic.message.contains("obtener")
        }),
        "{:?}",
        parsed.diagnostics
    );
    let generated = oxido_compiler::generate_ox(&source, LexOptions::default())
        .unwrap_or_else(|diagnostics| panic!("{diagnostics:?}"));
    assert!(generated.rust.contains("fn obtener"), "{}", generated.rust);
    assert!(
        generated.rust.contains("repo.obtener()"),
        "{}",
        generated.rust
    );
    assert!(
        generated.rust.contains("valores.get(0)"),
        "{}",
        generated.rust
    );

    let result = oxido_compiler::build_ox(&source, LexOptions::default(), CargoAction::Run)
        .unwrap_or_else(|diagnostics| panic!("{diagnostics:?}"));
    assert!(result.success, "{} {:?}", result.stderr, result.diagnostics);
    assert!(
        result
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code.as_deref() == Some("OX3002")),
        "{:?}",
        result.diagnostics
    );
    assert_eq!(result.program_output, ["7"]);
}

#[test]
fn texto_is_a_variable_name_and_a_type_alias_only_in_type_position() {
    use oxido_compiler::application::cargo::CargoAction;
    use oxido_language::domain::syntax::LexOptions;

    let source = SourceFile::new(
        "sintaxis indentacion\nestructura Dato:\n    valor: &'static texto\n    propio: Cadena\nfuncion principal():\n    dejar texto = Cadena::from(\"hola\")\n    dejar vista: &texto = &texto\n    dejar dato = Dato { valor: \"hola\", propio: texto.clonar() }\n    imprimir!(\"{} {} {}\", texto, vista, dato.valor)\n",
    );
    let generated = oxido_compiler::generate_ox(&source, LexOptions::default())
        .unwrap_or_else(|diagnostics| panic!("{diagnostics:?}"));
    assert!(
        generated.rust.contains("let texto = String::from"),
        "{}",
        generated.rust
    );
    assert!(
        generated.rust.contains("valor: &'static str"),
        "{}",
        generated.rust
    );
    assert!(
        generated.rust.contains("propio: texto"),
        "{}",
        generated.rust
    );
    assert!(generated.rust.contains("vista: &str"), "{}", generated.rust);
    let result = oxido_compiler::build_ox(&source, LexOptions::default(), CargoAction::Run)
        .unwrap_or_else(|diagnostics| panic!("{diagnostics:?}"));
    assert!(result.success, "{} {:?}", result.stderr, result.diagnostics);
    assert_eq!(result.program_output, ["hola hola hola"]);
}

#[test]
fn error_trait_name_is_not_rewritten_as_the_err_variant_alias() {
    use oxido_compiler::application::cargo::CargoAction;
    use oxido_language::domain::syntax::LexOptions;

    let source = SourceFile::new(
        "sintaxis llaves\nuse std::error::Error;\nfn fallar() -> Resultado<(), Caja<dyn Error>> { Ok(()) }\nfn main() { fallar().unwrap(); }\n",
    );
    let generated = oxido_compiler::generate_ox(&source, LexOptions::default())
        .unwrap_or_else(|diagnostics| panic!("{diagnostics:?}"));
    assert!(generated.rust.contains("dyn Error"), "{}", generated.rust);
    assert!(
        generated.rust.contains("Result<(), Box<dyn Error>>"),
        "{}",
        generated.rust
    );
    let result = oxido_compiler::build_ox(&source, LexOptions::default(), CargoAction::Check)
        .unwrap_or_else(|diagnostics| panic!("{diagnostics:?}"));
    assert!(result.success, "{} {:?}", result.stderr, result.diagnostics);
}
