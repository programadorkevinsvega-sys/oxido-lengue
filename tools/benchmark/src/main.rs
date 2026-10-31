#![forbid(unsafe_code)]

//! Mide análisis, generación, memoria y latencia del LSP con entradas reproducibles.
//!
//! Cada escenario se ejecuta en un proceso hijo para que el pico de memoria no se
//! contamine con los escenarios anteriores. El proceso padre solo agrega resultados.

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    env,
    process::Command,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::{Duration, Instant},
};
use sysinfo::{Pid, ProcessesToUpdate, System};

const ANALYSIS_BUDGET: Duration = Duration::from_secs(10);
const MAX_SOURCE_BYTES: usize = 4 * 1024 * 1024;
const TYPICAL_LINES: usize = 200;
const SCENARIOS: [&str; 13] = [
    "lex-typical",
    "parse-typical",
    "generate-typical",
    "lex-large",
    "parse-large",
    "generate-large",
    "lex-invalid",
    "parse-invalid",
    "lsp-open-typical",
    "lsp-hover-typical",
    "lsp-symbols-typical",
    "lsp-references-typical",
    "lsp-open-large",
];

#[derive(Clone, Copy)]
struct Iterations {
    warmup: usize,
    measured: usize,
}

#[derive(Serialize, Deserialize)]
struct Measurement {
    scenario: String,
    input_bytes: usize,
    iterations: usize,
    min_ms: f64,
    median_ms: f64,
    p95_ms: f64,
    max_ms: f64,
    peak_rss_kib: u64,
    detail: Value,
}

#[derive(Serialize, Deserialize)]
struct Stats {
    min_ms: f64,
    median_ms: f64,
    p95_ms: f64,
    max_ms: f64,
}

enum Scenario {
    Lex {
        source: oxido_language::SourceFile,
        options: oxido_language::domain::syntax::LexOptions,
    },
    Parse {
        source: oxido_language::SourceFile,
        options: oxido_language::domain::syntax::LexOptions,
    },
    Generate {
        source: oxido_language::SourceFile,
        options: oxido_language::domain::syntax::LexOptions,
    },
    Lsp {
        state: Box<oxido_lsp::ServerState>,
        request: Value,
        input_bytes: usize,
    },
}

impl Scenario {
    fn run(&mut self) -> Result<Value, String> {
        match self {
            Self::Lex { source, options } => {
                let output = oxido_language::domain::syntax::lex(source, *options);
                Ok(json!({
                    "tokens": output.tokens.len(),
                    "diagnostics": output.diagnostics.len(),
                    "timed_out": output.timed_out,
                }))
            }
            Self::Parse { source, options } => {
                let output = oxido_language::domain::syntax::parse(source, *options);
                Ok(json!({
                    "items": output.file.as_ref().map_or(0, |file| file.items.len()),
                    "diagnostics": output.diagnostics.len(),
                    "timed_out": output.diagnostics.iter().any(|diagnostic| {
                        diagnostic.message.contains("límite de tiempo")
                    }),
                }))
            }
            Self::Generate { source, options } => {
                match oxido_compiler::generate_ox(source, *options) {
                    Ok(program) => Ok(json!({
                        "rust_bytes": program.rust.len(),
                        "diagnostics": program.diagnostics.len(),
                        "timed_out": false,
                    })),
                    Err(diagnostics) => Ok(json!({
                        "error_diagnostics": diagnostics.len(),
                        "timed_out": diagnostics.iter().any(|diagnostic| {
                            diagnostic.message.contains("límite de tiempo")
                        }),
                        "first_error": diagnostics.first().map(|diagnostic| json!({
                            "code": diagnostic.code,
                            "message": diagnostic.message,
                        })),
                    })),
                }
            }
            Self::Lsp { state, request, .. } => {
                let body = serde_json::to_vec(request)
                    .map_err(|error| format!("no se pudo serializar la solicitud LSP: {error}"))?;
                let messages = oxido_lsp::handle_json(&body, state.as_mut())
                    .map_err(|error| format!("solicitud LSP inválida: {error}"))?;
                if messages.is_empty() {
                    return Err("la solicitud LSP no produjo mensajes".to_owned());
                }
                if request.get("id").is_some() {
                    let result = messages
                        .iter()
                        .find_map(|message| message.get("result"))
                        .ok_or_else(|| "la respuesta LSP no incluye result".to_owned())?;
                    if result.is_null() {
                        return Err("la respuesta LSP tiene result nulo".to_owned());
                    }
                    return Ok(json!({
                        "messages": messages.len(),
                        "result_items": result.as_array().map_or(0, Vec::len),
                        "result_present": !result.is_null(),
                        "result": result,
                    }));
                }
                Ok(json!({"messages": messages.len()}))
            }
        }
    }
}

fn main() {
    if let Err(error) = run() {
        eprintln!("no se pudo ejecutar el benchmark: {error}");
        std::process::exit(2);
    }
}

fn run() -> Result<(), String> {
    let mut args = env::args().skip(1);
    match args.next().as_deref() {
        Some("--scenario") => {
            let name = args
                .next()
                .ok_or_else(|| "falta el nombre del escenario".to_owned())?;
            let measurement = run_scenario(&name)?;
            let encoded = serde_json::to_string(&measurement)
                .map_err(|error| format!("no se pudo serializar el resultado: {error}"))?;
            println!("{encoded}");
            Ok(())
        }
        Some("--check") => run_all(true),
        Some(other) => Err(format!("argumento no reconocido: {other}")),
        None => run_all(false),
    }
}

fn run_all(check: bool) -> Result<(), String> {
    let executable =
        env::current_exe().map_err(|error| format!("no se pudo resolver el binario: {error}"))?;
    let mut measurements = Vec::new();
    for name in SCENARIOS {
        let output = Command::new(&executable)
            .arg("--scenario")
            .arg(name)
            .output()
            .map_err(|error| format!("no se pudo ejecutar el escenario {name}: {error}"))?;
        if !output.status.success() {
            return Err(format!(
                "el escenario {name} falló: {}",
                String::from_utf8_lossy(&output.stderr).trim()
            ));
        }
        let measurement = serde_json::from_slice(&output.stdout).map_err(|error| {
            format!("el escenario {name} devolvió un resultado inválido: {error}")
        })?;
        measurements.push(measurement);
    }
    print_report(&measurements)?;
    if check {
        evaluate_objectives(&measurements)?;
    }
    Ok(())
}

fn run_scenario(name: &str) -> Result<Measurement, String> {
    let (mut scenario, iterations) = build_scenario(name)?;
    let input_bytes = scenario_input_bytes(&scenario);
    for _ in 0..iterations.warmup {
        let _detail = scenario.run()?;
    }
    let sampler = MemorySampler::start()?;
    let mut samples = Vec::with_capacity(iterations.measured);
    let mut detail = Value::Null;
    for _ in 0..iterations.measured {
        let started = Instant::now();
        detail = scenario.run()?;
        samples.push(started.elapsed());
    }
    let peak_rss_kib = sampler.finish()?;
    let stats = compute_stats(samples)?;
    Ok(Measurement {
        scenario: name.to_owned(),
        input_bytes,
        iterations: iterations.measured,
        min_ms: stats.min_ms,
        median_ms: stats.median_ms,
        p95_ms: stats.p95_ms,
        max_ms: stats.max_ms,
        peak_rss_kib,
        detail,
    })
}

fn scenario_input_bytes(scenario: &Scenario) -> usize {
    match scenario {
        Scenario::Lex { source, .. }
        | Scenario::Parse { source, .. }
        | Scenario::Generate { source, .. } => source.text().len(),
        Scenario::Lsp { input_bytes, .. } => *input_bytes,
    }
}

fn build_scenario(name: &str) -> Result<(Scenario, Iterations), String> {
    let options = oxido_language::domain::syntax::LexOptions {
        project_mode: Some(oxido_language::domain::syntax::SyntaxMode::Indentation),
    };
    let typical_text = typical_source(TYPICAL_LINES);
    match name {
        "lex-typical" => Ok((
            Scenario::Lex {
                source: oxido_language::SourceFile::new(typical_text),
                options,
            },
            Iterations {
                warmup: 3,
                measured: 20,
            },
        )),
        "parse-typical" => Ok((
            Scenario::Parse {
                source: oxido_language::SourceFile::new(typical_text),
                options,
            },
            Iterations {
                warmup: 3,
                measured: 20,
            },
        )),
        "generate-typical" => Ok((
            Scenario::Generate {
                source: oxido_language::SourceFile::new(typical_text),
                options,
            },
            Iterations {
                warmup: 2,
                measured: 10,
            },
        )),
        "lex-large" => Ok((
            Scenario::Lex {
                source: oxido_language::SourceFile::new(large_source()?),
                options,
            },
            Iterations {
                warmup: 1,
                measured: 3,
            },
        )),
        "parse-large" => Ok((
            Scenario::Parse {
                source: oxido_language::SourceFile::new(large_source()?),
                options,
            },
            Iterations {
                warmup: 1,
                measured: 3,
            },
        )),
        "generate-large" => Ok((
            Scenario::Generate {
                source: oxido_language::SourceFile::new(large_source()?),
                options,
            },
            Iterations {
                warmup: 1,
                measured: 2,
            },
        )),
        "lex-invalid" => Ok((
            Scenario::Lex {
                source: oxido_language::SourceFile::new(invalid_source()?),
                options,
            },
            Iterations {
                warmup: 1,
                measured: 3,
            },
        )),
        "parse-invalid" => Ok((
            Scenario::Parse {
                source: oxido_language::SourceFile::new(invalid_source()?),
                options,
            },
            Iterations {
                warmup: 1,
                measured: 3,
            },
        )),
        "lsp-open-typical" => {
            let state = initialized_state()?;
            Ok((
                Scenario::Lsp {
                    state,
                    request: open_request(&typical_text, 1),
                    input_bytes: typical_text.len(),
                },
                Iterations {
                    warmup: 1,
                    measured: 10,
                },
            ))
        }
        "lsp-hover-typical" => {
            let state = opened_state(&typical_text)?;
            Ok((
                Scenario::Lsp {
                    state,
                    request: position_request("textDocument/hover", 10),
                    input_bytes: typical_text.len(),
                },
                Iterations {
                    warmup: 5,
                    measured: 50,
                },
            ))
        }
        "lsp-symbols-typical" => {
            let state = opened_state(&typical_text)?;
            Ok((
                Scenario::Lsp {
                    state,
                    request: json!({
                        "jsonrpc":"2.0",
                        "id":11,
                        "method":"textDocument/documentSymbol",
                        "params":{"textDocument":{"uri":"file:///bench/main.ox"}},
                    }),
                    input_bytes: typical_text.len(),
                },
                Iterations {
                    warmup: 3,
                    measured: 20,
                },
            ))
        }
        "lsp-references-typical" => {
            let state = opened_state(&typical_text)?;
            Ok((
                Scenario::Lsp {
                    state,
                    request: json!({
                        "jsonrpc":"2.0",
                        "id":12,
                        "method":"textDocument/references",
                        "params":{
                            "textDocument":{"uri":"file:///bench/main.ox"},
                            "position":{"line":1,"character":8},
                            "context":{"includeDeclaration":true},
                        },
                    }),
                    input_bytes: typical_text.len(),
                },
                Iterations {
                    warmup: 3,
                    measured: 20,
                },
            ))
        }
        "lsp-open-large" => {
            let state = initialized_state()?;
            let source = large_source()?;
            Ok((
                Scenario::Lsp {
                    state,
                    request: open_request(&source, 1),
                    input_bytes: source.len(),
                },
                Iterations {
                    warmup: 0,
                    measured: 1,
                },
            ))
        }
        other => Err(format!("escenario desconocido: {other}")),
    }
}

fn typical_source(lines: usize) -> String {
    let mut source = String::from("sintaxis indentacion\nfuncion saludar():\n");
    for index in 0..lines {
        source.push_str(&format!("    dejar valor_{index} = {index}\n"));
    }
    source
}

fn large_source() -> Result<String, String> {
    const STATEMENTS_PER_FUNCTION: usize = 16;
    let mut source = String::from("sintaxis indentacion\n");
    let mut index = 0usize;
    loop {
        let mut block = format!("funcion funcion_{index}():\n");
        for _ in 0..STATEMENTS_PER_FUNCTION {
            block.push_str(&format!("    dejar valor_{index} = {index}\n"));
            index = index.saturating_add(1);
        }
        if source.len() + block.len() > MAX_SOURCE_BYTES {
            break;
        }
        source.push_str(&block);
    }
    if source.len() < MAX_SOURCE_BYTES / 2 {
        return Err("no se pudo construir la entrada grande".to_owned());
    }
    Ok(source)
}

fn invalid_source() -> Result<String, String> {
    let mut source = String::from("sintaxis llaves\n");
    while source.len() + 1 < MAX_SOURCE_BYTES {
        source.push('\u{1}');
    }
    Ok(source)
}

fn initialized_state() -> Result<Box<oxido_lsp::ServerState>, String> {
    let mut state = oxido_lsp::ServerState::default();
    let request = json!({
        "jsonrpc":"2.0",
        "id":1,
        "method":"initialize",
        "params":{"initializationOptions":{"syntaxMode":"indentation"}},
    });
    let body = serde_json::to_vec(&request)
        .map_err(|error| format!("no se pudo serializar initialize: {error}"))?;
    let response = oxido_lsp::handle_json(&body, &mut state)
        .map_err(|error| format!("initialize inválido: {error}"))?;
    if response.is_empty() {
        return Err("initialize no produjo respuesta".to_owned());
    }
    Ok(Box::new(state))
}

fn opened_state(source: &str) -> Result<Box<oxido_lsp::ServerState>, String> {
    let mut state = initialized_state()?;
    let request = open_request(source, 1);
    let body = serde_json::to_vec(&request)
        .map_err(|error| format!("no se pudo serializar didOpen: {error}"))?;
    let messages = oxido_lsp::handle_json(&body, state.as_mut())
        .map_err(|error| format!("didOpen inválido: {error}"))?;
    if messages.is_empty() {
        return Err("didOpen no produjo diagnósticos".to_owned());
    }
    Ok(state)
}

fn open_request(source: &str, version: i64) -> Value {
    json!({
        "jsonrpc":"2.0",
        "method":"textDocument/didOpen",
        "params":{
            "textDocument":{
                "uri":"file:///bench/main.ox",
                "languageId":"oxido",
                "version":version,
                "text":source,
            }
        },
    })
}

fn position_request(method: &str, id: i64) -> Value {
    json!({
        "jsonrpc":"2.0",
        "id":id,
        "method":method,
        "params":{
            "textDocument":{"uri":"file:///bench/main.ox"},
            "position":{"line":1,"character":8},
        },
    })
}

fn compute_stats(mut samples: Vec<Duration>) -> Result<Stats, String> {
    if samples.is_empty() {
        return Err("no hay muestras para calcular estadísticas".to_owned());
    }
    samples.sort();
    let count = samples.len();
    let median = if count.is_multiple_of(2) {
        (samples[count / 2 - 1] + samples[count / 2]) / 2
    } else {
        samples[count / 2]
    };
    let p95_index = ((count as f64 * 0.95).ceil() as usize)
        .saturating_sub(1)
        .min(count - 1);
    Ok(Stats {
        min_ms: to_millis(samples[0]),
        median_ms: to_millis(median),
        p95_ms: to_millis(samples[p95_index]),
        max_ms: to_millis(samples[count - 1]),
    })
}

fn to_millis(duration: Duration) -> f64 {
    duration.as_secs_f64() * 1_000.0
}

struct MemorySampler {
    stop: Arc<AtomicBool>,
    handle: Option<thread::JoinHandle<u64>>,
    fallback_kib: u64,
}

impl MemorySampler {
    fn start() -> Result<Self, String> {
        let pid = Pid::from_u32(std::process::id());
        let baseline = current_memory_kib(pid)
            .ok_or_else(|| "no se pudo leer la memoria inicial del proceso".to_owned())?;
        let stop = Arc::new(AtomicBool::new(false));
        let thread_stop = Arc::clone(&stop);
        let handle = thread::spawn(move || {
            let Ok(mut system) = System::new() else {
                return baseline;
            };
            let mut peak = baseline;
            while !thread_stop.load(Ordering::Relaxed) {
                system.refresh_processes(ProcessesToUpdate::Some(&[pid]), true);
                if let Some(process) = system.process(pid) {
                    peak = peak.max(process.memory() / 1024);
                }
                thread::sleep(Duration::from_millis(20));
            }
            peak
        });
        Ok(Self {
            stop,
            handle: Some(handle),
            fallback_kib: baseline,
        })
    }

    fn finish(mut self) -> Result<u64, String> {
        let pid = Pid::from_u32(std::process::id());
        let final_kib = current_memory_kib(pid).unwrap_or(self.fallback_kib);
        self.stop.store(true, Ordering::Relaxed);
        let sampled = match self.handle.take() {
            Some(handle) => handle
                .join()
                .map_err(|_| "el muestreador de memoria terminó con pánico".to_owned())?,
            None => self.fallback_kib,
        };
        Ok(sampled.max(final_kib))
    }
}

fn current_memory_kib(pid: Pid) -> Option<u64> {
    let mut system = System::new().ok()?;
    system.refresh_processes(ProcessesToUpdate::Some(&[pid]), true);
    system.process(pid).map(|process| process.memory() / 1024)
}

fn print_report(measurements: &[Measurement]) -> Result<(), String> {
    println!("# Benchmark de análisis y LSP\n");
    println!("- Sistema: `{}` / `{}`", env::consts::OS, env::consts::ARCH);
    println!(
        "- CPUs lógicas: {}",
        thread::available_parallelism().map_or(0, |value| value.get())
    );
    println!("- Rust: `{}`", rustc_version());
    println!("- Presupuesto de análisis: {} s", ANALYSIS_BUDGET.as_secs());
    println!(
        "- Método: un proceso hijo por escenario; RSS máximo muestreado cada 20 ms; entrada grande acotada a 4 MiB."
    );
    println!();
    println!(
        "| Escenario | Entrada | Iteraciones | Mín (ms) | Mediana (ms) | P95 (ms) | Máx (ms) | RSS máx (MiB) |"
    );
    println!("|---|---:|---:|---:|---:|---:|---:|---:|");
    for measurement in measurements {
        println!(
            "| `{}` | {} | {} | {:.2} | {:.2} | {:.2} | {:.2} | {:.1} |",
            measurement.scenario,
            format_bytes(measurement.input_bytes),
            measurement.iterations,
            measurement.min_ms,
            measurement.median_ms,
            measurement.p95_ms,
            measurement.max_ms,
            measurement.peak_rss_kib as f64 / 1024.0,
        );
    }
    println!("\n## Detalle\n");
    for measurement in measurements {
        println!("- `{}`: `{}`", measurement.scenario, measurement.detail);
    }
    print_objectives(measurements);
    Ok(())
}

struct Objective {
    p95_ms: f64,
    rss_mib: u64,
}

fn objective(scenario: &str) -> Option<Objective> {
    match scenario {
        "lex-typical" => Some(Objective {
            p95_ms: 50.0,
            rss_mib: 64,
        }),
        "parse-typical" => Some(Objective {
            p95_ms: 50.0,
            rss_mib: 64,
        }),
        "generate-typical" => Some(Objective {
            p95_ms: 500.0,
            rss_mib: 128,
        }),
        "lex-large" => Some(Objective {
            p95_ms: 2_000.0,
            rss_mib: 128,
        }),
        "parse-large" => Some(Objective {
            p95_ms: 2_000.0,
            rss_mib: 256,
        }),
        "generate-large" => Some(Objective {
            p95_ms: 10_000.0,
            rss_mib: 1_536,
        }),
        "lex-invalid" | "parse-invalid" => Some(Objective {
            p95_ms: 2_000.0,
            rss_mib: 128,
        }),
        "lsp-open-typical" => Some(Objective {
            p95_ms: 100.0,
            rss_mib: 128,
        }),
        "lsp-hover-typical" | "lsp-symbols-typical" | "lsp-references-typical" => Some(Objective {
            p95_ms: 50.0,
            rss_mib: 128,
        }),
        "lsp-open-large" => Some(Objective {
            p95_ms: 3_000.0,
            rss_mib: 512,
        }),
        _ => None,
    }
}

fn detail_u64(detail: &Value, key: &str) -> Option<u64> {
    detail.get(key).and_then(Value::as_u64)
}

fn detail_bool(detail: &Value, key: &str) -> Option<bool> {
    detail.get(key).and_then(Value::as_bool)
}

/// Comprueba que el escenario hizo el trabajo esperado y no solo que fuera rápido.
fn functional_failure(measurement: &Measurement) -> Option<String> {
    let detail = &measurement.detail;
    let failure = match measurement.scenario.as_str() {
        "lex-typical" | "lex-large" => {
            if detail_u64(detail, "tokens").unwrap_or(0) == 0 {
                Some("no se tokenizó ninguna entrada")
            } else if detail_u64(detail, "diagnostics").unwrap_or(0) != 0 {
                Some("la entrada válida produjo diagnósticos")
            } else {
                None
            }
        }
        "lex-invalid" => {
            if detail_u64(detail, "diagnostics").unwrap_or(0) == 0 {
                Some("la entrada inválida no produjo diagnósticos")
            } else {
                None
            }
        }
        "parse-typical" | "parse-large" => {
            if detail_u64(detail, "items").unwrap_or(0) == 0 {
                Some("no se produjo ningún elemento del AST")
            } else if detail_u64(detail, "diagnostics").unwrap_or(0) != 0 {
                Some("la entrada válida produjo diagnósticos")
            } else {
                None
            }
        }
        "parse-invalid" => {
            if detail_u64(detail, "diagnostics").unwrap_or(0) == 0 {
                Some("la entrada inválida no produjo diagnósticos")
            } else {
                None
            }
        }
        "generate-typical" | "generate-large" => {
            if detail.get("error_diagnostics").is_some() {
                Some("la generación devolvió diagnósticos de error")
            } else if detail_u64(detail, "rust_bytes").unwrap_or(0) == 0 {
                Some("no se generó Rust")
            } else if detail_u64(detail, "diagnostics").unwrap_or(0) != 0 {
                Some("la generación produjo diagnósticos")
            } else {
                None
            }
        }
        "lsp-open-typical" | "lsp-open-large" => {
            if detail_u64(detail, "messages").unwrap_or(0) == 0 {
                Some("la apertura LSP no produjo mensajes")
            } else {
                None
            }
        }
        "lsp-hover-typical" => {
            if detail_bool(detail, "result_present") != Some(true) {
                Some("hover no devolvió resultado")
            } else {
                None
            }
        }
        "lsp-symbols-typical" | "lsp-references-typical" => {
            if detail_u64(detail, "result_items").unwrap_or(0) == 0 {
                Some("la consulta LSP no devolvió elementos")
            } else {
                None
            }
        }
        _ => None,
    };
    failure.map(str::to_owned)
}

fn print_objectives(measurements: &[Measurement]) {
    println!("\n## Objetivos\n");
    println!(
        "| Escenario | P95 observado (ms) | Límite P95 (ms) | RSS observado (MiB) | Límite RSS (MiB) | Estado |"
    );
    println!("|---|---:|---:|---:|---:|:---:|");
    for measurement in measurements {
        let Some(objective) = objective(&measurement.scenario) else {
            continue;
        };
        let rss_mib = measurement.peak_rss_kib as f64 / 1024.0;
        let passed = measurement.p95_ms <= objective.p95_ms
            && rss_mib <= objective.rss_mib as f64
            && detail_bool(&measurement.detail, "timed_out") != Some(true)
            && functional_failure(measurement).is_none();
        println!(
            "| `{}` | {:.2} | {:.0} | {:.1} | {} | {} |",
            measurement.scenario,
            measurement.p95_ms,
            objective.p95_ms,
            rss_mib,
            objective.rss_mib,
            if passed { "Aprobado" } else { "Fallido" }
        );
    }
}

fn evaluate_objectives(measurements: &[Measurement]) -> Result<(), String> {
    let mut failures = Vec::new();
    for measurement in measurements {
        if let Some(objective) = objective(&measurement.scenario) {
            let rss_mib = measurement.peak_rss_kib as f64 / 1024.0;
            if measurement.p95_ms > objective.p95_ms {
                failures.push(format!(
                    "{}: P95 {:.2} ms supera {:.0} ms",
                    measurement.scenario, measurement.p95_ms, objective.p95_ms
                ));
            }
            if rss_mib > objective.rss_mib as f64 {
                failures.push(format!(
                    "{}: RSS {:.1} MiB supera {} MiB",
                    measurement.scenario, rss_mib, objective.rss_mib
                ));
            }
        }
        if detail_bool(&measurement.detail, "timed_out") == Some(true) {
            failures.push(format!(
                "{}: el análisis superó el plazo de 10 s",
                measurement.scenario
            ));
        }
        if let Some(reason) = functional_failure(measurement) {
            failures.push(format!("{}: {reason}", measurement.scenario));
        }
    }
    if failures.is_empty() {
        println!("\nObjetivos: todos aprobados.");
        Ok(())
    } else {
        Err(failures.join("; "))
    }
}

fn format_bytes(bytes: usize) -> String {
    if bytes >= 1024 * 1024 {
        format!("{:.1} MiB", bytes as f64 / (1024.0 * 1024.0))
    } else if bytes >= 1024 {
        format!("{:.1} KiB", bytes as f64 / 1024.0)
    } else {
        format!("{bytes} B")
    }
}

fn rustc_version() -> String {
    Command::new("rustc")
        .arg("--version")
        .output()
        .ok()
        .filter(|output| output.status.success())
        .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_owned())
        .unwrap_or_else(|| "desconocida".to_owned())
}
