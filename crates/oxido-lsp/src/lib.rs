#![forbid(unsafe_code)]

mod symbols;

use oxido_compiler::application::source_map::SourceMap;
use oxido_language::{
    SourceFile,
    domain::syntax::{LexOptions, TokenKind, lex},
};
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, BTreeSet, HashMap},
    io::{self, BufRead, BufReader, Read, Write},
    path::PathBuf,
    process::{Child, ChildStdin, ChildStdout, Command, Stdio},
    sync::{
        Arc, Mutex, OnceLock,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver, Sender, SyncSender, TrySendError},
    },
    thread,
    time::{Duration, Instant},
};
use symbols::{SymbolKind, declared_symbols};

pub const MAX_MESSAGE_BYTES: usize = 4 * 1024 * 1024;
const MAX_HEADER_LINE_BYTES: usize = 8 * 1024;
const MAX_PENDING_SEMANTIC_REQUESTS: usize = 4;
const MAX_PENDING_CARGO_REQUESTS: usize = 1;
const MAX_QUEUED_LSP_EVENTS: usize = 8;
const MAX_OPEN_DOCUMENTS: usize = 64;
const MAX_OPEN_DOCUMENT_BYTES: usize = 16 * 1024 * 1024;
const MAX_SINGLE_DOCUMENT_BYTES: usize = 4 * 1024 * 1024;
const MAX_QUEUED_DIAGNOSTIC_TASKS: usize = 1;
// A request is bounded, but the first rust-analyzer query has to start and
// index a minimal Cargo workspace.  This budget gives that cold start a real
// chance to answer while preserving the local Óxido fallback for slow cases.
const RUST_ANALYZER_RESPONSE_TIMEOUT: Duration = Duration::from_millis(900);
const RUST_ANALYZER_START_TIMEOUT: Duration = Duration::from_secs(2);
const CANCELED_ERROR_CODE: i64 = -32800;
const CONTENT_MODIFIED_ERROR_CODE: i64 = -32801;
static NEXT_RA_WORKSPACE: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
static RA_STARTS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
static RA_RESPONSES: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
static RA_WORKER: OnceLock<Mutex<Option<SyncSender<RaWork>>>> = OnceLock::new();

struct RustAnalyzerSession {
    child: Child,
    input: Arc<Mutex<ChildStdin>>,
    output: Receiver<Value>,
    root: PathBuf,
    files: BTreeMap<PathBuf, String>,
    next_id: i64,
}

#[derive(Clone)]
struct RustAnalyzerWorkspace {
    files: BTreeMap<PathBuf, String>,
    source_maps: BTreeMap<PathBuf, RustAnalyzerSourceMap>,
    requested_path: PathBuf,
}

#[derive(Clone)]
struct RustAnalyzerSourceMap {
    uri: String,
    text: String,
    map: SourceMap,
}

#[derive(Clone, Debug)]
struct WorkspaceSymbol {
    uri: String,
    name: String,
    kind: SymbolKind,
    container: Option<String>,
    module_path: Vec<String>,
    selection: Value,
    range: Value,
}

struct RaWork {
    workspace: RustAnalyzerWorkspace,
    method: String,
    params: Value,
    reply: Sender<Option<Value>>,
}

#[derive(Default)]
pub struct ServerState {
    documents: BTreeMap<String, TextDocument>,
    project_mode: Option<oxido_language::domain::syntax::SyntaxMode>,
    project_root: Option<PathBuf>,
    trusted_workspace: bool,
    total_document_bytes: usize,
    diagnostic_runs: u64,
    last_diagnostic_ms: Option<u128>,
}

struct TextDocument {
    version: Option<i64>,
    text: String,
}

struct PendingSemanticRequest {
    id: Value,
    uri: Option<String>,
    document_version: Option<i64>,
    document_text: String,
    cancellation: Arc<AtomicBool>,
}

struct DiagnosticTask {
    uri: String,
    version: Option<i64>,
    text: String,
    project_mode: Option<oxido_language::domain::syntax::SyntaxMode>,
    cancellation: Arc<AtomicBool>,
}

struct PendingDiagnostic {
    version: Option<i64>,
    cancellation: Arc<AtomicBool>,
}

struct PendingCargoRequest {
    id: Value,
    cancellation: Arc<AtomicBool>,
    snapshot: CargoCheckSnapshot,
}

#[derive(Clone, Debug)]
struct CargoCheckSnapshot {
    uri: String,
    document_version: Option<i64>,
    document_text: String,
    source_path: Option<PathBuf>,
    project_root: Option<PathBuf>,
    project_mode: Option<oxido_language::domain::syntax::SyntaxMode>,
}

struct CargoCheckOutcome {
    result: Result<
        oxido_compiler::application::cargo::CargoBuildResult,
        Vec<oxido_language::domain::diagnostics::Diagnostic>,
    >,
    dependency_count: usize,
    lockfile_present: bool,
}

enum ServerEvent {
    Frame(Vec<u8>),
    AsyncResponse {
        key: String,
        response: Value,
    },
    CargoReady {
        key: String,
        outcome: CargoCheckOutcome,
    },
    DiagnosticReady {
        uri: String,
        version: Option<i64>,
        cancellation: Arc<AtomicBool>,
        notification: Option<Value>,
        elapsed_ms: u128,
    },
    InputEnded,
    InputFailed(io::Error),
}

/// Runs the stdio LSP transport and permits cancellation while semantic tokens are computed.
pub fn serve_stdio(input: impl Read + Send + 'static, mut output: impl Write) -> io::Result<()> {
    let (events_tx, events_rx) = mpsc::sync_channel(MAX_QUEUED_LSP_EVENTS);
    let reader_tx = events_tx.clone();
    let reader = thread::Builder::new()
        .name("oxido-lsp-reader".to_owned())
        .spawn(move || {
            let mut input = BufReader::new(input);
            loop {
                match read_frame(&mut input) {
                    Ok(Some(frame)) => {
                        if reader_tx.send(ServerEvent::Frame(frame)).is_err() {
                            break;
                        }
                    }
                    Ok(None) => {
                        if let Err(error) = reader_tx.send(ServerEvent::InputEnded) {
                            eprintln!("LSP event receiver closed: {error}");
                        }
                        break;
                    }
                    Err(error) => {
                        if let Err(send_error) = reader_tx.send(ServerEvent::InputFailed(error)) {
                            eprintln!("LSP event receiver closed: {send_error}");
                        }
                        break;
                    }
                }
            }
        })?;

    let (diagnostics_tx, diagnostics_rx) =
        mpsc::sync_channel::<DiagnosticTask>(MAX_QUEUED_DIAGNOSTIC_TASKS);
    let diagnostics_events_tx = events_tx.clone();
    let diagnostics_worker = thread::Builder::new()
        .name("oxido-lsp-diagnostics".to_owned())
        .spawn(move || {
            while let Ok(task) = diagnostics_rx.recv() {
                let started = Instant::now();
                let notification = if task.cancellation.load(Ordering::Relaxed) {
                    None
                } else {
                    Some(diagnostic_notification(
                        &task.uri,
                        &task.text,
                        task.project_mode,
                    ))
                };
                if let Err(error) = diagnostics_events_tx.send(ServerEvent::DiagnosticReady {
                    uri: task.uri,
                    version: task.version,
                    cancellation: task.cancellation,
                    notification,
                    elapsed_ms: started.elapsed().as_millis(),
                }) {
                    eprintln!("LSP diagnostic event receiver closed: {error}");
                    break;
                }
            }
        })?;

    let mut state = ServerState::default();
    let mut pending = HashMap::<String, PendingSemanticRequest>::new();
    let mut pending_cargo = HashMap::<String, PendingCargoRequest>::new();
    let mut pending_diagnostics = HashMap::<String, PendingDiagnostic>::new();
    let mut dirty_diagnostics = BTreeSet::<String>::new();
    let mut input_error = None;
    let mut input_ended = false;
    while let Ok(event) = events_rx.recv() {
        match event {
            ServerEvent::Frame(body) => {
                let request = serde_json::from_slice::<Value>(&body).map_err(io::Error::other)?;
                if cancel_request(&request, &pending, &pending_cargo) {
                    continue;
                }
                if request.get("method").and_then(Value::as_str) == Some("oxido/check") {
                    let id = request.get("id").cloned().unwrap_or(Value::Null);
                    let key = id.to_string();
                    let snapshot = match cargo_check_snapshot(&request, &state) {
                        Ok(snapshot) => snapshot,
                        Err(response) => {
                            write_lsp_message(&mut output, &response)?;
                            continue;
                        }
                    };
                    if pending_cargo.contains_key(&key) {
                        write_lsp_message(
                            &mut output,
                            &json!({"jsonrpc":"2.0","id":id,"error":{"code":-32600,"message":"id de solicitud duplicado"}}),
                        )?;
                        continue;
                    }
                    if pending_cargo.len() >= MAX_PENDING_CARGO_REQUESTS {
                        write_lsp_message(
                            &mut output,
                            &json!({"jsonrpc":"2.0","id":id,"error":{"code":-32002,"message":"ya hay una comprobación Cargo en curso"}}),
                        )?;
                        continue;
                    }
                    let cancellation = Arc::new(AtomicBool::new(false));
                    let worker_cancellation = Arc::clone(&cancellation);
                    let worker_tx = events_tx.clone();
                    let worker_key = key.clone();
                    let worker_snapshot = snapshot.clone();
                    let worker = thread::Builder::new()
                        .name("oxido-lsp-cargo-check".to_owned())
                        .spawn(move || {
                            let outcome = run_cargo_check(worker_snapshot, &worker_cancellation);
                            if let Err(error) = worker_tx.send(ServerEvent::CargoReady {
                                key: worker_key,
                                outcome,
                            }) {
                                eprintln!("LSP Cargo event receiver closed: {error}");
                            }
                        });
                    match worker {
                        Ok(_) => {
                            pending_cargo.insert(
                                key,
                                PendingCargoRequest {
                                    id,
                                    cancellation,
                                    snapshot,
                                },
                            );
                        }
                        Err(error) => write_lsp_message(
                            &mut output,
                            &json!({"jsonrpc":"2.0","id":id,"error":{"code":-32603,"message":format!("no se pudo iniciar la comprobación Cargo: {error}")}}),
                        )?,
                    }
                    continue;
                }
                if request.get("method").and_then(Value::as_str) == Some("oxido/stats") {
                    let id = request.get("id").cloned().unwrap_or(Value::Null);
                    write_lsp_message(
                        &mut output,
                        &json!({"jsonrpc":"2.0","id":id,"result":{"openDocuments":state.documents.len(),"documentBytes":state.total_document_bytes,"diagnosticRuns":state.diagnostic_runs,"lastDiagnosticMs":state.last_diagnostic_ms,"residentMemoryKiB":resident_memory_kib(),"rustAnalyzerResponses":RA_RESPONSES.load(Ordering::Relaxed)}}),
                    )?;
                    continue;
                }
                if request.get("method").and_then(Value::as_str)
                    == Some("textDocument/semanticTokens/full")
                    && let Some(id) = request.get("id").cloned()
                {
                    let key = id.to_string();
                    if pending.contains_key(&key) {
                        write_lsp_message(
                            &mut output,
                            &json!({"jsonrpc":"2.0","id":id,"error":{"code":-32600,"message":"id de solicitud duplicado"}}),
                        )?;
                        continue;
                    }
                    if pending.len() >= MAX_PENDING_SEMANTIC_REQUESTS {
                        write_lsp_message(
                            &mut output,
                            &json!({"jsonrpc":"2.0","id":id,"error":{"code":-32002,"message":"límite de solicitudes LSP concurrentes alcanzado"}}),
                        )?;
                        continue;
                    }
                    let uri = request
                        .pointer("/params/textDocument/uri")
                        .and_then(Value::as_str)
                        .map(str::to_owned);
                    let document = uri.as_deref().and_then(|uri| state.documents.get(uri));
                    let document_text =
                        document.map_or_else(String::new, |document| document.text.clone());
                    let document_version = document.and_then(|document| document.version);
                    let cancellation = Arc::new(AtomicBool::new(false));
                    let worker_cancellation = Arc::clone(&cancellation);
                    let worker_tx = events_tx.clone();
                    let worker_text = document_text.clone();
                    let worker_id = id.clone();
                    let worker_key = key.clone();
                    let project_mode = state.project_mode;
                    let worker = thread::Builder::new()
                        .name("oxido-lsp-semantic-tokens".to_owned())
                        .spawn(move || {
                            let response = match semantic_tokens_cancellable(
                                &worker_text,
                                project_mode,
                                &worker_cancellation,
                            ) {
                                Some(data) => {
                                    json!({"jsonrpc":"2.0","id":worker_id,"result":{"data":data}})
                                }
                                None => canceled_response(worker_id),
                            };
                            if let Err(error) = worker_tx.send(ServerEvent::AsyncResponse {
                                key: worker_key,
                                response,
                            }) {
                                eprintln!("LSP response receiver closed: {error}");
                            }
                        });
                    match worker {
                        Ok(_) => {
                            pending.insert(
                                key,
                                PendingSemanticRequest {
                                    id,
                                    uri,
                                    document_version,
                                    document_text,
                                    cancellation,
                                },
                            );
                        }
                        Err(error) => write_lsp_message(
                            &mut output,
                            &json!({"jsonrpc":"2.0","id":id,"error":{"code":-32603,"message":format!("no se pudo iniciar el trabajo LSP: {error}")}}),
                        )?,
                    }
                    continue;
                }
                let changed_uri = request
                    .pointer("/params/textDocument/uri")
                    .and_then(Value::as_str)
                    .filter(|_| {
                        matches!(
                            request.get("method").and_then(Value::as_str),
                            Some("textDocument/didOpen" | "textDocument/didChange")
                        )
                    })
                    .map(str::to_owned);
                let closed_uri = request
                    .pointer("/params/textDocument/uri")
                    .and_then(Value::as_str)
                    .filter(|_| {
                        request.get("method").and_then(Value::as_str)
                            == Some("textDocument/didClose")
                    })
                    .map(str::to_owned);
                let responses = handle(request, &mut state);
                let rejected_for_resources = responses.iter().any(|response| {
                    response
                        .pointer("/params/diagnostics/0/code")
                        .and_then(Value::as_str)
                        .is_some_and(|code| matches!(code, "OX6001" | "OX6002" | "OX6003"))
                });
                for response in responses {
                    write_lsp_message(&mut output, &response)?;
                }
                if let Some(uri) = closed_uri {
                    if let Some(previous) = pending_diagnostics.remove(&uri) {
                        previous.cancellation.store(true, Ordering::Relaxed);
                    }
                    dirty_diagnostics.remove(&uri);
                    write_lsp_message(
                        &mut output,
                        &json!({"jsonrpc":"2.0","method":"textDocument/publishDiagnostics","params":{"uri":uri,"diagnostics":[]}}),
                    )?;
                }
                if let Some(uri) = changed_uri {
                    if let Some(previous) = pending_diagnostics.remove(&uri) {
                        previous.cancellation.store(true, Ordering::Relaxed);
                    }
                    if rejected_for_resources {
                        dirty_diagnostics.remove(&uri);
                    } else {
                        dirty_diagnostics.insert(uri);
                        dispatch_diagnostic(
                            &diagnostics_tx,
                            &state,
                            &mut dirty_diagnostics,
                            &mut pending_diagnostics,
                        );
                    }
                }
            }
            ServerEvent::AsyncResponse { key, response } => {
                if let Some(request) = pending.remove(&key) {
                    let final_response = if request.cancellation.load(Ordering::Relaxed) {
                        canceled_response(request.id)
                    } else if semantic_request_was_modified(&state, &request) {
                        json!({"jsonrpc":"2.0","id":request.id,"error":{"code":CONTENT_MODIFIED_ERROR_CODE,"message":"el documento cambió durante el cálculo"}})
                    } else {
                        response
                    };
                    if !input_ended {
                        write_lsp_message(&mut output, &final_response)?;
                    }
                }
                if input_ended
                    && pending.is_empty()
                    && pending_cargo.is_empty()
                    && pending_diagnostics.is_empty()
                {
                    break;
                }
            }
            ServerEvent::CargoReady { key, outcome } => {
                if let Some(request) = pending_cargo.remove(&key) {
                    let final_response = if request.cancellation.load(Ordering::Relaxed) {
                        canceled_response(request.id)
                    } else if document_was_modified(
                        &state,
                        &request.snapshot.uri,
                        request.snapshot.document_version,
                        &request.snapshot.document_text,
                    ) {
                        json!({"jsonrpc":"2.0","id":request.id,"error":{"code":CONTENT_MODIFIED_ERROR_CODE,"message":"el documento cambió durante la comprobación Cargo"}})
                    } else {
                        cargo_check_outcome_response(request.id, outcome, &state, &request.snapshot)
                    };
                    if !input_ended {
                        write_lsp_message(&mut output, &final_response)?;
                    }
                }
                if input_ended
                    && pending.is_empty()
                    && pending_cargo.is_empty()
                    && pending_diagnostics.is_empty()
                {
                    break;
                }
            }
            ServerEvent::DiagnosticReady {
                uri,
                version,
                cancellation,
                notification,
                elapsed_ms,
            } => {
                state.diagnostic_runs = state.diagnostic_runs.saturating_add(1);
                state.last_diagnostic_ms = Some(elapsed_ms);
                let is_current_task = pending_diagnostics.get(&uri).is_some_and(|diagnostic| {
                    diagnostic.version == version
                        && Arc::ptr_eq(&diagnostic.cancellation, &cancellation)
                });
                if is_current_task {
                    let diagnostic = pending_diagnostics.remove(&uri);
                    let is_canceled = diagnostic
                        .as_ref()
                        .is_none_or(|item| item.cancellation.load(Ordering::Relaxed));
                    let document_is_current = state
                        .documents
                        .get(&uri)
                        .is_some_and(|document| document.version == version);
                    if !input_ended
                        && !is_canceled
                        && document_is_current
                        && let Some(notification) = notification
                    {
                        write_lsp_message(&mut output, &notification)?;
                    }
                }
                // `dirty_diagnostics` is populated only by didOpen/didChange.
                // Re-inserting an unchanged URI here would create an endless
                // publishDiagnostics loop and starve interactive requests.
                dispatch_diagnostic(
                    &diagnostics_tx,
                    &state,
                    &mut dirty_diagnostics,
                    &mut pending_diagnostics,
                );
                if input_ended
                    && pending.is_empty()
                    && pending_cargo.is_empty()
                    && pending_diagnostics.is_empty()
                {
                    break;
                }
            }
            ServerEvent::InputEnded => {
                input_ended = true;
                for request in pending.values() {
                    request.cancellation.store(true, Ordering::Relaxed);
                }
                for request in pending_cargo.values() {
                    request.cancellation.store(true, Ordering::Relaxed);
                }
                for diagnostic in pending_diagnostics.values() {
                    diagnostic.cancellation.store(true, Ordering::Relaxed);
                }
                dirty_diagnostics.clear();
                if pending.is_empty() && pending_cargo.is_empty() && pending_diagnostics.is_empty()
                {
                    break;
                }
            }
            ServerEvent::InputFailed(error) => {
                input_error = Some(error);
                input_ended = true;
                for request in pending.values() {
                    request.cancellation.store(true, Ordering::Relaxed);
                }
                for request in pending_cargo.values() {
                    request.cancellation.store(true, Ordering::Relaxed);
                }
                for diagnostic in pending_diagnostics.values() {
                    diagnostic.cancellation.store(true, Ordering::Relaxed);
                }
                dirty_diagnostics.clear();
                if pending.is_empty() && pending_cargo.is_empty() && pending_diagnostics.is_empty()
                {
                    break;
                }
            }
        }
    }
    for request in pending.values() {
        request.cancellation.store(true, Ordering::Relaxed);
    }
    for request in pending_cargo.values() {
        request.cancellation.store(true, Ordering::Relaxed);
    }
    reader
        .join()
        .map_err(|_| io::Error::other("el hilo lector LSP terminó inesperadamente"))?;
    drop(diagnostics_tx);
    diagnostics_worker
        .join()
        .map_err(|_| io::Error::other("el worker de diagnósticos LSP terminó inesperadamente"))?;
    shutdown_rust_analyzer_worker();
    if let Some(error) = input_error {
        return Err(error);
    }
    Ok(())
}

fn write_lsp_message(output: &mut impl Write, message: &Value) -> io::Result<()> {
    let encoded = serde_json::to_vec(message).map_err(io::Error::other)?;
    write!(output, "Content-Length: {}\r\n\r\n", encoded.len())?;
    output.write_all(&encoded)?;
    output.flush()
}

fn cancel_request(
    request: &Value,
    pending: &HashMap<String, PendingSemanticRequest>,
    pending_cargo: &HashMap<String, PendingCargoRequest>,
) -> bool {
    if request.get("method").and_then(Value::as_str) != Some("$/cancelRequest") {
        return false;
    }
    if let Some(id) = request.pointer("/params/id") {
        let key = id.to_string();
        if let Some(request) = pending.get(&key) {
            request.cancellation.store(true, Ordering::Relaxed);
        }
        if let Some(request) = pending_cargo.get(&key) {
            request.cancellation.store(true, Ordering::Relaxed);
        }
    }
    true
}

fn canceled_response(id: Value) -> Value {
    json!({"jsonrpc":"2.0","id":id,"error":{"code":CANCELED_ERROR_CODE,"message":"solicitud cancelada"}})
}

fn document_was_modified(state: &ServerState, uri: &str, version: Option<i64>, text: &str) -> bool {
    state
        .documents
        .get(uri)
        .is_none_or(|document| document.version != version || document.text != text)
}

fn semantic_request_was_modified(state: &ServerState, request: &PendingSemanticRequest) -> bool {
    request.uri.as_deref().is_some_and(|uri| {
        document_was_modified(state, uri, request.document_version, &request.document_text)
    })
}

fn dispatch_diagnostic(
    sender: &SyncSender<DiagnosticTask>,
    state: &ServerState,
    dirty: &mut BTreeSet<String>,
    pending: &mut HashMap<String, PendingDiagnostic>,
) {
    if pending.len() >= MAX_QUEUED_DIAGNOSTIC_TASKS {
        return;
    }
    while let Some(uri) = dirty.iter().next().cloned() {
        let Some(document) = state.documents.get(&uri) else {
            dirty.remove(&uri);
            continue;
        };
        let cancellation = Arc::new(AtomicBool::new(false));
        let task = DiagnosticTask {
            uri: uri.clone(),
            version: document.version,
            text: document.text.clone(),
            project_mode: state.project_mode,
            cancellation: Arc::clone(&cancellation),
        };
        match sender.try_send(task) {
            Ok(()) => {
                dirty.remove(&uri);
                pending.insert(
                    uri,
                    PendingDiagnostic {
                        version: document.version,
                        cancellation,
                    },
                );
                return;
            }
            Err(TrySendError::Full(task)) => {
                drop(task);
                return;
            }
            Err(TrySendError::Disconnected(task)) => {
                drop(task);
                eprintln!("LSP diagnostic worker disconnected before scheduling a task");
                return;
            }
        }
    }
}

pub fn validate_frame_length(length: usize) -> io::Result<()> {
    if length > MAX_MESSAGE_BYTES {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "mensaje LSP supera el límite de 4 MiB",
        ));
    }
    Ok(())
}

pub fn read_frame(input: &mut impl BufRead) -> io::Result<Option<Vec<u8>>> {
    let mut content_length = None;
    let mut line = Vec::new();
    loop {
        let has_line = read_bounded_header_line(input, &mut line)?;
        if !has_line {
            if line.is_empty() && content_length.is_none() {
                return Ok(None);
            }
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "cabecera LSP incompleta",
            ));
        }
        if line == b"\r\n" || line == b"\n" {
            break;
        }
        if let Some(value) = line.strip_prefix(b"Content-Length:") {
            if content_length.is_some() {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "Content-Length duplicado",
                ));
            }
            let value = std::str::from_utf8(value)
                .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
            let length = value
                .trim()
                .parse::<usize>()
                .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
            validate_frame_length(length)?;
            content_length = Some(length);
        }
    }

    let length = content_length
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "falta Content-Length"))?;
    let mut body = vec![0_u8; length];
    input.read_exact(&mut body)?;
    Ok(Some(body))
}

fn read_bounded_header_line(input: &mut impl BufRead, line: &mut Vec<u8>) -> io::Result<bool> {
    line.clear();
    loop {
        let available = input.fill_buf()?;
        if available.is_empty() {
            return Ok(!line.is_empty());
        }
        let newline = available.iter().position(|byte| *byte == b'\n');
        let bytes_to_consume = newline.map_or(available.len(), |position| position + 1);
        if line.len().saturating_add(bytes_to_consume) > MAX_HEADER_LINE_BYTES {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "línea de cabecera LSP supera 8 KiB",
            ));
        }
        line.extend_from_slice(&available[..bytes_to_consume]);
        input.consume(bytes_to_consume);
        if newline.is_some() {
            return Ok(true);
        }
    }
}

/// Decodes one JSON-RPC payload and dispatches it through the LSP state machine.
pub fn handle_json(body: &[u8], state: &mut ServerState) -> Result<Vec<Value>, serde_json::Error> {
    let request: Value = serde_json::from_slice(body)?;
    let mut messages = handle(request.clone(), state);
    if matches!(
        request.get("method").and_then(Value::as_str),
        Some("textDocument/didOpen" | "textDocument/didChange")
    ) && let Some(uri) = request
        .pointer("/params/textDocument/uri")
        .and_then(Value::as_str)
        && let Some(document) = state.documents.get(uri)
    {
        messages.push(diagnostic_notification(
            uri,
            &document.text,
            state.project_mode,
        ));
    }
    Ok(messages)
}

fn handle(request: Value, state: &mut ServerState) -> Vec<Value> {
    let Some(method) = request.get("method").and_then(Value::as_str) else {
        return Vec::new();
    };
    let id = request.get("id").cloned();
    let mut extra_messages = Vec::new();
    let response = match method {
        "initialize" => {
            state.project_mode = project_syntax_mode(&request);
            state.trusted_workspace = request
                .pointer("/params/initializationOptions/trusted")
                .and_then(Value::as_bool)
                == Some(true);
            state.project_root = request
                .pointer("/params/rootUri")
                .and_then(Value::as_str)
                .and_then(file_uri_path)
                .map(PathBuf::from)
                .and_then(|path| std::fs::canonicalize(path).ok())
                .filter(|path| path.is_dir());
            id.map(|id| json!({"jsonrpc":"2.0","id":id,"result":{"capabilities":{"textDocumentSync":{"openClose":true,"change":2},"semanticTokensProvider":{"full":true,"legend":{"tokenTypes":["keyword","function","variable","string","number","comment","type","operator"],"tokenModifiers":[]}},"completionProvider":{"triggerCharacters":[".",":"]},"hoverProvider":true,"documentSymbolProvider":true,"definitionProvider":true,"referencesProvider":true,"inlayHintProvider":true,"documentFormattingProvider":true}}}))
        },
        "shutdown" => id.map(|id| json!({"jsonrpc":"2.0","id":id,"result":null})),
        "textDocument/semanticTokens/full" => id.map(|id| semantic_response(id, request.get("params"), state)),
        "textDocument/completion" => id.map(|id| completion_response(id, request.get("params"), state)),
        "textDocument/hover" => id.map(|id| hover_response(id, request.get("params"), state)),
        "textDocument/documentSymbol" => id.map(|id| document_symbols_response(id, request.get("params"), state)),
        "textDocument/definition" => id.map(|id| definition_response(id, request.get("params"), state)),
        "textDocument/references" => id.map(|id| references_response(id, request.get("params"), state)),
        "textDocument/formatting" => id.map(|id| formatting_response(id, request.get("params"), state)),
        "textDocument/inlayHint" => id.map(|id| inlay_hint_response(id, request.get("params"), state)),
        "textDocument/didOpen" => { extra_messages.extend(open_document(&request, state)); None }
        "textDocument/didChange" => { extra_messages.extend(change_document(&request, state)); None }
        "textDocument/didClose" => { close_document(&request, state); None }
        "exit" => None,
        _ => id.map(|id| json!({"jsonrpc":"2.0","id":id,"error":{"code":-32601,"message":"método LSP no soportado"}})),
    };
    let mut messages = response.into_iter().collect::<Vec<_>>();
    messages.append(&mut extra_messages);
    messages
}

fn diagnostic_notification(
    uri: &str,
    text: &str,
    project_mode: Option<oxido_language::domain::syntax::SyntaxMode>,
) -> Value {
    let source = SourceFile::new(text);
    let result = oxido_compiler::parse_ox(&source, LexOptions { project_mode });
    let diagnostics = result.diagnostics.into_iter().filter_map(|diagnostic| {
        let start = lsp_position(text, diagnostic.primary_span.start())?;
        let end = lsp_position(text, diagnostic.primary_span.end()).unwrap_or(start.clone());
        let severity = match diagnostic.severity {
            oxido_language::domain::diagnostics::Severity::Error => 1,
            oxido_language::domain::diagnostics::Severity::Warning => 2,
            oxido_language::domain::diagnostics::Severity::Info => 3,
            oxido_language::domain::diagnostics::Severity::Hint => 4,
        };
        Some(json!({"range":{"start":start,"end":end},"severity":severity,"message":diagnostic.message,"code":diagnostic.code,"source":"oxido"}))
    }).collect::<Vec<_>>();
    json!({"jsonrpc":"2.0","method":"textDocument/publishDiagnostics","params":{"uri":uri,"diagnostics":diagnostics}})
}

fn cargo_check_snapshot(request: &Value, state: &ServerState) -> Result<CargoCheckSnapshot, Value> {
    let id = request.get("id").cloned().unwrap_or(Value::Null);
    let trusted = state.trusted_workspace
        && request.pointer("/params/trusted").and_then(Value::as_bool) == Some(true);
    if !trusted {
        return Err(
            json!({"jsonrpc":"2.0","id":id,"error":{"code":-32003,"message":"Cargo requiere confianza explícita del workspace"}}),
        );
    }
    let Some(uri) = request
        .pointer("/params/textDocument/uri")
        .and_then(Value::as_str)
    else {
        return Err(
            json!({"jsonrpc":"2.0","id":id,"error":{"code":-32602,"message":"falta textDocument.uri"}}),
        );
    };
    let Some(document) = state.documents.get(uri) else {
        return Err(
            json!({"jsonrpc":"2.0","id":id,"error":{"code":-32602,"message":"el documento no está abierto"}}),
        );
    };
    Ok(CargoCheckSnapshot {
        uri: uri.to_owned(),
        document_version: document.version,
        document_text: document.text.clone(),
        source_path: file_uri_path(uri).map(PathBuf::from),
        project_root: state.project_root.clone(),
        project_mode: state.project_mode,
    })
}

fn run_cargo_check(snapshot: CargoCheckSnapshot, cancellation: &AtomicBool) -> CargoCheckOutcome {
    let source = snapshot.source_path.as_deref().map_or_else(
        || SourceFile::new(&snapshot.document_text),
        |path| SourceFile::with_path(path, &snapshot.document_text),
    );
    let options = LexOptions {
        project_mode: snapshot.project_mode,
    };
    let mut dependency_count = 0_usize;
    let mut lockfile_present = false;
    let result = if let Some(project_root) = snapshot.project_root.as_deref() {
        let config_path = project_root.join("oxido.toml");
        let lockfile_path = project_root.join("Cargo.lock");
        lockfile_present = lockfile_path.is_file();
        let config_text = std::fs::read_to_string(&config_path).map_err(|error| error.to_string());
        match config_text.and_then(|text| {
            oxido_compiler::config::ProjectConfig::parse(&text).map_err(|error| error.to_string())
        }) {
            Ok(config) => match config.resolve_dependencies(project_root) {
                Ok(dependencies) => {
                    dependency_count = dependencies.len();
                    oxido_compiler::build_ox_project_in_project_cancellable(
                        &source,
                        options,
                        &dependencies,
                        Some(&lockfile_path),
                        oxido_compiler::application::cargo::CargoAction::CheckTrusted,
                        project_root,
                        Some(cancellation),
                    )
                }
                Err(error) => Err(vec![
                    oxido_language::domain::diagnostics::Diagnostic::error(
                        "OX7003",
                        error,
                        source
                            .span(0, 0)
                            .unwrap_or_else(oxido_language::domain::source::Span::empty),
                    ),
                ]),
            },
            Err(error) => Err(vec![
                oxido_language::domain::diagnostics::Diagnostic::error(
                    "OX7003",
                    format!("no se pudo leer oxido.toml: {error}"),
                    source
                        .span(0, 0)
                        .unwrap_or_else(oxido_language::domain::source::Span::empty),
                ),
            ]),
        }
    } else {
        oxido_compiler::build_ox_in_project_cancellable(
            &source,
            options,
            &[],
            None,
            oxido_compiler::application::cargo::CargoAction::CheckTrusted,
            Some(cancellation),
        )
    };
    CargoCheckOutcome {
        result,
        dependency_count,
        lockfile_present,
    }
}

fn cargo_check_outcome_response(
    id: Value,
    outcome: CargoCheckOutcome,
    state: &ServerState,
    snapshot: &CargoCheckSnapshot,
) -> Value {
    let (success, timed_out, exit_code, files) = match outcome.result {
        Ok(result) => (
            result.success,
            result.timed_out,
            result.exit_code,
            cargo_diagnostics_by_uri(
                &result.diagnostics,
                result.success,
                state,
                &snapshot.uri,
                &snapshot.document_text,
            ),
        ),
        Err(diagnostics) => (
            false,
            false,
            None,
            compiler_diagnostics_by_uri(&diagnostics, &snapshot.uri, &snapshot.document_text),
        ),
    };
    json!({
        "jsonrpc":"2.0",
        "id":id,
        "result":{
            "success":success,
            "timedOut":timed_out,
            "exitCode":exit_code,
            "dependencyCount":outcome.dependency_count,
            "lockfilePresent":outcome.lockfile_present,
            "files":files
        }
    })
}

fn cargo_diagnostic_severity(level: &str) -> u8 {
    match level {
        "error" | "failure-note" => 1,
        "warning" => 2,
        "note" => 3,
        "help" => 4,
        _ => 1,
    }
}

fn safe_lsp_position(text: &str, offset: usize) -> Option<Value> {
    let mut offset = offset.min(text.len());
    while offset > 0 && !text.is_char_boundary(offset) {
        offset -= 1;
    }
    lsp_position(text, offset)
}

fn source_text_for_diagnostic(
    diagnostic: &oxido_compiler::application::cargo::MappedCargoDiagnostic,
    state: &ServerState,
    fallback_uri: &str,
    fallback_text: &str,
) -> (String, String) {
    let Some(relative) = diagnostic.source_file.as_deref() else {
        return (fallback_uri.to_owned(), fallback_text.to_owned());
    };
    let Some(project_root) = state.project_root.as_deref() else {
        return (fallback_uri.to_owned(), fallback_text.to_owned());
    };
    let relative = std::path::Path::new(relative);
    if relative.is_absolute() {
        return (fallback_uri.to_owned(), fallback_text.to_owned());
    }
    let Ok(canonical) = project_root.join(relative).canonicalize() else {
        return (fallback_uri.to_owned(), fallback_text.to_owned());
    };
    if !canonical.starts_with(project_root) {
        return (fallback_uri.to_owned(), fallback_text.to_owned());
    }
    let Ok(metadata) = std::fs::symlink_metadata(&canonical) else {
        return (fallback_uri.to_owned(), fallback_text.to_owned());
    };
    if !metadata.is_file() || metadata.len() > MAX_SINGLE_DOCUMENT_BYTES as u64 {
        return (fallback_uri.to_owned(), fallback_text.to_owned());
    }
    let uri = file_uri(&canonical);
    let text = state
        .documents
        .get(&uri)
        .map(|document| document.text.clone())
        .or_else(|| std::fs::read_to_string(&canonical).ok());
    text.map_or_else(
        || (fallback_uri.to_owned(), fallback_text.to_owned()),
        |text| (uri, text),
    )
}

fn cargo_diagnostics_by_uri(
    diagnostics: &[oxido_compiler::application::cargo::MappedCargoDiagnostic],
    success: bool,
    state: &ServerState,
    fallback_uri: &str,
    fallback_text: &str,
) -> Vec<Value> {
    let mut groups = BTreeMap::<String, Vec<Value>>::new();
    for diagnostic in diagnostics {
        if diagnostic.level == "failure-note" {
            continue;
        }
        let (uri, text) =
            source_text_for_diagnostic(diagnostic, state, fallback_uri, fallback_text);
        let start = diagnostic
            .source
            .and_then(|location| safe_lsp_position(&text, location.start))
            .unwrap_or_else(|| json!({"line":0,"character":0}));
        let end = diagnostic
            .source
            .and_then(|location| safe_lsp_position(&text, location.end))
            .unwrap_or_else(|| start.clone());
        groups.entry(uri).or_default().push(json!({
            "range":{"start":start,"end":end},
            "severity":cargo_diagnostic_severity(&diagnostic.level),
            "code":diagnostic.code,
            "source":"cargo",
            "message":diagnostic.message
        }));
    }
    if groups.is_empty() && !success {
        groups
            .entry(fallback_uri.to_owned())
            .or_default()
            .push(json!({
                "range":{"start":{"line":0,"character":0},"end":{"line":0,"character":0}},
                "severity":1,
                "code":"OX5001",
                "source":"cargo",
                "message":"Cargo terminó sin un diagnóstico mapeable"
            }));
    }
    groups
        .into_iter()
        .map(|(uri, diagnostics)| json!({"uri":uri,"diagnostics":diagnostics}))
        .collect()
}

fn compiler_diagnostics_by_uri(
    diagnostics: &[oxido_language::domain::diagnostics::Diagnostic],
    uri: &str,
    text: &str,
) -> Vec<Value> {
    let diagnostics = diagnostics
        .iter()
        .filter_map(|diagnostic| {
            let start = safe_lsp_position(text, diagnostic.primary_span.start())?;
            let end = safe_lsp_position(text, diagnostic.primary_span.end())
                .unwrap_or_else(|| start.clone());
            let severity = match diagnostic.severity {
                oxido_language::domain::diagnostics::Severity::Error => 1,
                oxido_language::domain::diagnostics::Severity::Warning => 2,
                oxido_language::domain::diagnostics::Severity::Info => 3,
                oxido_language::domain::diagnostics::Severity::Hint => 4,
            };
            Some(json!({
                "range":{"start":start,"end":end},
                "severity":severity,
                "code":diagnostic.code,
                "source":"oxido",
                "message":diagnostic.message
            }))
        })
        .collect::<Vec<_>>();
    vec![json!({"uri":uri,"diagnostics":diagnostics})]
}

fn completion_response(id: Value, params: Option<&Value>, state: &ServerState) -> Value {
    let uri = params
        .and_then(|params| params.pointer("/textDocument/uri"))
        .and_then(Value::as_str);
    let text = uri
        .and_then(|uri| state.documents.get(uri))
        .map_or("", |document| document.text.as_str());
    if let Some(response) =
        rust_analyzer_response("textDocument/completion", &id, params, state, text)
    {
        return response;
    }
    let mut labels = BTreeSet::new();
    for (alias, _, _) in oxido_language::domain::syntax::token::KEYWORD_ALIASES {
        labels.insert(*alias);
    }
    for alias in [
        "imprimir!",
        "imprimir_sin_salto!",
        "formatear!",
        "vector!",
        "afirmar!",
        "estructura",
        "enumeracion",
        "rasgo",
        "implementar",
        "modulo",
        "usar",
    ] {
        labels.insert(alias);
    }
    let mut items = labels
        .into_iter()
        .map(|label| json!({"label":label,"kind":14,"detail":"Palabra clave o alias de Óxido"}))
        .collect::<Vec<_>>();
    let current_uri = uri.unwrap_or_default();
    let mut offered = BTreeSet::new();
    if let Some(document) = state.documents.get(current_uri) {
        for symbol in declared_symbols(&document.text, state.project_mode) {
            if offered.insert((symbol.name.clone(), current_uri.to_owned())) {
                items.push(completion_item(
                    &symbol.name,
                    symbol.kind,
                    "Declaración en este documento",
                ));
            }
        }
    }
    for symbol in workspace_symbols(state) {
        if symbol.uri == current_uri || !offered.insert((symbol.name.clone(), symbol.uri.clone())) {
            continue;
        }
        let detail = if symbol.module_path.is_empty() {
            "Declaración en otro módulo .ox".to_owned()
        } else {
            format!(
                "Declaración en otro módulo .ox: {}",
                symbol.module_path.join("::")
            )
        };
        items.push(completion_item(&symbol.name, symbol.kind, &detail));
    }
    json!({"jsonrpc":"2.0","id":id,"result":{"isIncomplete":false,"items":items}})
}

fn completion_item(name: &str, kind: SymbolKind, detail: &str) -> Value {
    json!({
        "label":name,
        "kind":completion_kind(kind),
        "detail":detail,
    })
}

fn completion_kind(kind: SymbolKind) -> u8 {
    match kind {
        SymbolKind::Function => 3,
        SymbolKind::Method => 2,
        SymbolKind::Struct => 22,
        SymbolKind::Enum => 13,
        SymbolKind::Trait => 8,
        SymbolKind::TypeAlias => 25,
        SymbolKind::Module => 9,
    }
}

fn hover_response(id: Value, params: Option<&Value>, state: &ServerState) -> Value {
    let Some((uri, text, offset)) = request_document_position(params, state) else {
        return json!({"jsonrpc":"2.0","id":id,"result":null});
    };
    if let Some(response) = rust_analyzer_response("textDocument/hover", &id, params, state, text) {
        return response;
    }
    let Some(token) = lex_token_at(text, offset, state.project_mode) else {
        return json!({"jsonrpc":"2.0","id":id,"result":null});
    };
    let token_text = &text[token.span.start()..token.span.end()];
    let contents = match &token.kind {
        TokenKind::Keyword { rust_keyword, .. } => Some(format!(
            "`{token_text}` es el alias Óxido de Rust `{rust_keyword}`."
        )),
        TokenKind::Identifier { .. } => {
            let candidates =
                resolve_symbol_candidates(state, &uri, text, token.span.start(), token_text);
            match candidates.as_slice() {
                [] => None,
                [symbol] => Some(format!(
                    "Declaración de {} `{token_text}` en `{}`.\n\nRango: `{}`–`{}`.",
                    symbol_kind_label(symbol.kind),
                    symbol.uri,
                    symbol.range["start"],
                    symbol.range["end"]
                )),
                symbols => {
                    let mut value = format!(
                        "`{token_text}` tiene {} declaraciones candidatas:\n",
                        symbols.len()
                    );
                    for symbol in symbols {
                        let location = if symbol.module_path.is_empty() {
                            symbol.uri.clone()
                        } else {
                            format!("{} ({})", symbol.uri, symbol.module_path.join("::"))
                        };
                        value.push_str(&format!(
                            "\n- {} en `{location}`",
                            symbol_kind_label(symbol.kind)
                        ));
                    }
                    Some(value)
                }
            }
        }
        _ => None,
    };
    let Some(contents) = contents else {
        return json!({"jsonrpc":"2.0","id":id,"result":null});
    };
    let start = lsp_position(text, token.span.start()).unwrap_or(json!({"line":0,"character":0}));
    let end = lsp_position(text, token.span.end()).unwrap_or(start.clone());
    json!({"jsonrpc":"2.0","id":id,"result":{"contents":{"kind":"markdown","value":contents},"range":{"start":start,"end":end}}})
}

fn symbol_kind_label(kind: SymbolKind) -> &'static str {
    match kind {
        SymbolKind::Function => "función",
        SymbolKind::Method => "método",
        SymbolKind::Struct => "estructura",
        SymbolKind::Enum => "enumeración",
        SymbolKind::Trait => "rasgo",
        SymbolKind::TypeAlias => "alias de tipo",
        SymbolKind::Module => "módulo",
    }
}

fn document_symbols_response(id: Value, params: Option<&Value>, state: &ServerState) -> Value {
    let Some(uri) = params
        .and_then(|params| params.pointer("/textDocument/uri"))
        .and_then(Value::as_str)
    else {
        return json!({"jsonrpc":"2.0","id":id,"result":[]});
    };
    let Some(document) = state.documents.get(uri) else {
        return json!({"jsonrpc":"2.0","id":id,"result":[]});
    };
    if state.trusted_workspace
        && let Some(response) = rust_analyzer_response(
            "textDocument/documentSymbol",
            &id,
            params,
            state,
            &document.text,
        )
    {
        return response;
    }
    let symbols = declared_symbols(&document.text, state.project_mode)
        .into_iter()
        .filter_map(|symbol| {
            let selection_start = lsp_position(&document.text, symbol.name_start)?;
            let selection_end = lsp_position(&document.text, symbol.name_end)?;
            let range_start = lsp_position(&document.text, symbol.range_start)?;
            let range_end = lsp_position(&document.text, symbol.range_end)?;
            Some(json!({
                "name":symbol.name,
                "kind":document_symbol_kind(symbol.kind),
                "range":{"start":range_start,"end":range_end},
                "selectionRange":{"start":selection_start,"end":selection_end},
                "detail":"Óxido"
            }))
        })
        .collect::<Vec<_>>();
    json!({"jsonrpc":"2.0","id":id,"result":symbols})
}

fn document_symbol_kind(kind: SymbolKind) -> u8 {
    match kind {
        SymbolKind::Function => 12,
        SymbolKind::Method => 6,
        SymbolKind::Struct => 23,
        SymbolKind::Enum => 10,
        SymbolKind::Trait => 11,
        SymbolKind::TypeAlias => 26,
        SymbolKind::Module => 2,
    }
}

fn definition_response(id: Value, params: Option<&Value>, state: &ServerState) -> Value {
    let Some((uri, text, offset)) = request_document_position(params, state) else {
        return json!({"jsonrpc":"2.0","id":id,"result":[]});
    };
    if let Some(response) =
        rust_analyzer_response("textDocument/definition", &id, params, state, text)
        && has_navigation_result(&response)
    {
        return response;
    }
    let Some(token) = lex_token_at(text, offset, state.project_mode) else {
        return json!({"jsonrpc":"2.0","id":id,"result":[]});
    };
    let name = &text[token.span.start()..token.span.end()];
    let mut seen = BTreeSet::new();
    let locations = resolve_symbol_candidates(state, &uri, text, token.span.start(), name)
        .into_iter()
        .filter_map(|symbol| {
            let key = (symbol.uri.clone(), symbol.selection.to_string());
            seen.insert(key)
                .then(|| json!({"uri":symbol.uri,"range":symbol.selection}))
        })
        .collect::<Vec<_>>();
    json!({"jsonrpc":"2.0","id":id,"result":locations})
}

fn resolve_symbol_candidates(
    state: &ServerState,
    current_uri: &str,
    text: &str,
    token_start: usize,
    name: &str,
) -> Vec<WorkspaceSymbol> {
    let qualifier = module_qualifier_before(text, token_start);
    let segments = qualifier.map(qualifier_segments).unwrap_or_default();
    let candidates = workspace_symbols(state)
        .into_iter()
        .filter(|symbol| symbol.name == name)
        .filter(|symbol| segments.is_empty() || matches_qualifier(symbol, &segments))
        .collect::<Vec<_>>();
    let local = candidates
        .iter()
        .filter(|symbol| symbol.uri == current_uri)
        .cloned()
        .collect::<Vec<_>>();
    if local.is_empty() { candidates } else { local }
}

fn matches_qualifier(symbol: &WorkspaceSymbol, segments: &[&str]) -> bool {
    if segments
        .last()
        .is_some_and(|last| symbol.container.as_deref() == Some(*last))
    {
        return module_suffix_matches(&symbol.module_path, &segments[..segments.len() - 1]);
    }
    module_suffix_matches(&symbol.module_path, segments)
}

fn module_suffix_matches(module_path: &[String], qualifier: &[&str]) -> bool {
    if qualifier.is_empty() {
        return true;
    }
    if qualifier.len() > module_path.len() {
        return false;
    }
    module_path[module_path.len() - qualifier.len()..]
        .iter()
        .zip(qualifier)
        .all(|(component, qualifier)| component == qualifier)
}

fn qualifier_segments(qualifier: &str) -> Vec<&str> {
    let mut segments = qualifier
        .split("::")
        .filter(|segment| !segment.is_empty())
        .collect::<Vec<_>>();
    while segments.first().is_some_and(|segment| {
        matches!(
            *segment,
            "crate" | "paquete" | "self" | "mismo" | "Self" | "Mismo" | "super" | "superior"
        )
    }) {
        segments.remove(0);
    }
    segments
}

fn module_qualifier_before(text: &str, token_start: usize) -> Option<&str> {
    let before = text.get(..token_start)?.trim_end();
    let without_separator = before.strip_suffix("::")?;
    let qualifier_start = without_separator
        .char_indices()
        .rev()
        .find_map(|(index, character)| {
            (!(character.is_ascii_alphanumeric() || character == '_' || character == ':'))
                .then_some(index + character.len_utf8())
        })
        .unwrap_or(0);
    let qualifier = without_separator.get(qualifier_start..)?.trim_end();
    (!qualifier.is_empty()).then_some(qualifier)
}

/// `rust-analyzer` can legitimately return an empty navigation result while a
/// freshly-created generated workspace is still being indexed.  Óxido already
/// has a bounded project index for declared functions, so an empty auxiliary
/// result must not hide a definition that index can resolve.
fn has_navigation_result(response: &Value) -> bool {
    response.get("result").is_some_and(|result| match result {
        Value::Array(locations) => !locations.is_empty(),
        Value::Null => false,
        _ => true,
    })
}

fn references_response(id: Value, params: Option<&Value>, state: &ServerState) -> Value {
    let Some((uri, text, offset)) = request_document_position(params, state) else {
        return json!({"jsonrpc":"2.0","id":id,"result":[]});
    };
    if let Some(response) =
        rust_analyzer_response("textDocument/references", &id, params, state, text)
    {
        return response;
    }
    let Some(selected) = lex_token_at(text, offset, state.project_mode) else {
        return json!({"jsonrpc":"2.0","id":id,"result":[]});
    };
    let selected_text = &text[selected.span.start()..selected.span.end()];
    let candidates =
        resolve_symbol_candidates(state, &uri, text, selected.span.start(), selected_text);
    let target_uris = candidates
        .iter()
        .map(|symbol| symbol.uri.as_str())
        .collect::<BTreeSet<_>>();
    let mut locations = Vec::new();
    for (module_uri, module_text, _, _) in workspace_documents(state) {
        let lexed = lex(
            &SourceFile::new(&module_text),
            LexOptions {
                project_mode: state.project_mode,
            },
        );
        let imports_candidate = module_uri != uri
            && document_imports_candidate(&module_text, selected_text, &candidates);
        for token in lexed.tokens {
            if !matches!(token.kind, TokenKind::Identifier { .. })
                || &module_text[token.span.start()..token.span.end()] != selected_text
            {
                continue;
            }
            let include = module_uri == uri
                || target_uris.contains(module_uri.as_str())
                || imports_candidate
                || module_qualifier_before(&module_text, token.span.start()).is_some_and(
                    |qualifier| {
                        let segments = qualifier_segments(qualifier);
                        candidates
                            .iter()
                            .any(|symbol| matches_qualifier(symbol, &segments))
                    },
                );
            if !include {
                continue;
            }
            let (Some(start), Some(end)) = (
                lsp_position(&module_text, token.span.start()),
                lsp_position(&module_text, token.span.end()),
            ) else {
                continue;
            };
            locations.push(json!({"uri":module_uri,"range":{"start":start,"end":end}}));
        }
    }
    json!({"jsonrpc":"2.0","id":id,"result":locations})
}

fn document_imports_candidate(text: &str, name: &str, candidates: &[WorkspaceSymbol]) -> bool {
    text.lines().any(|line| {
        let trimmed = line.trim_start();
        let Some(rest) = trimmed
            .strip_prefix("use ")
            .or_else(|| trimmed.strip_prefix("usar "))
        else {
            return false;
        };
        rest.contains(name)
            && candidates.iter().any(|candidate| {
                let path = candidate.module_path.join("::");
                !path.is_empty() && rest.contains(&path)
            })
    })
}

fn formatting_response(id: Value, params: Option<&Value>, state: &ServerState) -> Value {
    let Some(uri) = params
        .and_then(|value| value.pointer("/textDocument/uri"))
        .and_then(Value::as_str)
    else {
        return json!({"jsonrpc":"2.0","id":id,"result":[]});
    };
    let Some(document) = state.documents.get(uri) else {
        return json!({"jsonrpc":"2.0","id":id,"result":[]});
    };
    let source = SourceFile::new(&document.text);
    let lexed = lex(
        &source,
        LexOptions {
            project_mode: state.project_mode,
        },
    );
    if lexed.mode == Some(oxido_language::domain::syntax::SyntaxMode::Indentation) {
        let formatted =
            oxido_compiler::application::format::format_indented_source(&source, &lexed);
        if formatted == document.text {
            return json!({"jsonrpc":"2.0","id":id,"result":[]});
        }
        let end = lsp_position(&document.text, document.text.len())
            .unwrap_or(json!({"line":0,"character":0}));
        return json!({"jsonrpc":"2.0","id":id,"result":[{"range":{"start":{"line":0,"character":0},"end":end},"newText":formatted}]});
    }
    let Ok(generated) = oxido_compiler::generate_ox(
        &source,
        LexOptions {
            project_mode: state.project_mode,
        },
    ) else {
        return json!({"jsonrpc":"2.0","id":id,"result":[]});
    };
    let Ok(formatted) = oxido_compiler::import_rust(&SourceFile::new(generated.rust)) else {
        return json!({"jsonrpc":"2.0","id":id,"result":[]});
    };
    if formatted == document.text {
        return json!({"jsonrpc":"2.0","id":id,"result":[]});
    }
    let end = lsp_position(&document.text, document.text.len())
        .unwrap_or(json!({"line":0,"character":0}));
    json!({"jsonrpc":"2.0","id":id,"result":[{"range":{"start":{"line":0,"character":0},"end":end},"newText":formatted}]})
}

#[cfg(target_os = "linux")]
fn resident_memory_kib() -> Option<u64> {
    let status = std::fs::read_to_string("/proc/self/status").ok()?;
    status.lines().find_map(|line| {
        let value = line
            .strip_prefix("VmRSS:")?
            .trim()
            .strip_suffix("kB")?
            .trim();
        value.parse().ok()
    })
}

#[cfg(not(target_os = "linux"))]
fn resident_memory_kib() -> Option<u64> {
    None
}

fn inlay_hint_response(id: Value, params: Option<&Value>, state: &ServerState) -> Value {
    let Some(uri) = params
        .and_then(|value| value.pointer("/textDocument/uri"))
        .and_then(Value::as_str)
    else {
        return json!({"jsonrpc":"2.0","id":id,"result":[]});
    };
    let Some(document) = state.documents.get(uri) else {
        return json!({"jsonrpc":"2.0","id":id,"result":[]});
    };
    if state.trusted_workspace
        && let Some(response) =
            rust_analyzer_response("textDocument/inlayHint", &id, params, state, &document.text)
    {
        return response;
    }
    let mut hints = Vec::new();
    for (line_number, line) in document.text.lines().enumerate() {
        let Some(rest) = line.trim_start().strip_prefix("dejar ") else {
            continue;
        };
        let Some((name, initializer)) = rest.split_once('=') else {
            continue;
        };
        let name = name.trim();
        let value = initializer.trim().trim_end_matches(';').trim();
        let inferred = if value.starts_with('"') {
            "&str"
        } else if value == "verdadero" || value == "falso" || value == "true" || value == "false" {
            "bool"
        } else if value.parse::<i64>().is_ok() {
            "i32"
        } else if value.parse::<f64>().is_ok() {
            "f64"
        } else {
            continue;
        };
        let character = line.find(name).unwrap_or(0) + name.len();
        hints.push(json!({"position":{"line":line_number,"character":character},"label":format!(": {inferred}"),"kind":1,"paddingLeft":true}));
    }
    json!({"jsonrpc":"2.0","id":id,"result":hints})
}

fn rust_analyzer_response(
    method: &str,
    id: &Value,
    params: Option<&Value>,
    state: &ServerState,
    text: &str,
) -> Option<Value> {
    if !state.trusted_workspace {
        return None;
    }
    let uri = params
        .and_then(|value| value.pointer("/textDocument/uri"))
        .and_then(Value::as_str)?;
    let source = file_uri_path(uri)
        .map(PathBuf::from)
        .filter(|path| path.is_file())
        .map_or_else(
            || SourceFile::new(text),
            |path| SourceFile::with_path(path, text),
        );
    let generated = oxido_compiler::generate_ox(
        &source,
        LexOptions {
            project_mode: state.project_mode,
        },
    )
    .ok()?;
    let workspace = rust_analyzer_workspace(state, uri, &source, generated)?;
    let requested_rust = workspace.files.get(&workspace.requested_path)?;
    let generated_position = params
        .and_then(|value| value.get("position"))
        .and_then(|position| position_at_generated(text, requested_rust, position))?;
    let mut request_params = params.cloned().unwrap_or_else(|| json!({}));
    if let Some(document) = request_params.get_mut("textDocument") {
        *document =
            json!({"uri":format!("file:///src/{}", workspace.requested_path.to_string_lossy())});
    }
    if let Some(position) = generated_position
        && let Some(object) = request_params.as_object_mut()
    {
        object.insert("position".to_owned(), position);
    }
    // rust-analyzer is an auxiliary service.  It can be indexing a crate or a
    // procedural macro, so a delayed response must never hold the LSP event
    // loop hostage.  The local Óxido answer below each caller remains the
    // bounded fallback when this short query budget expires.
    let method = method.to_owned();
    let (reply_tx, reply_rx) = mpsc::sync_channel(1);
    thread::Builder::new()
        .name("oxido-lsp-rust-analyzer-query".to_owned())
        .spawn(move || {
            let response = run_rust_analyzer(workspace, &method, &Value::Null, request_params);
            if reply_tx.send(response).is_err() {
                eprintln!("la respuesta tardía de rust-analyzer ya no era necesaria");
            }
        })
        .ok()?;
    let mut response = reply_rx
        .recv_timeout(RUST_ANALYZER_RESPONSE_TIMEOUT)
        .ok()??;
    if let Some(object) = response.as_object_mut() {
        object.insert("id".to_owned(), id.clone());
    }
    Some(response)
}

fn rust_analyzer_workspace(
    state: &ServerState,
    source_uri: &str,
    source: &SourceFile,
    generated: oxido_compiler::GeneratedProgram,
) -> Option<RustAnalyzerWorkspace> {
    let mut files = BTreeMap::new();
    files.insert(PathBuf::from("main.rs"), generated.rust.clone());
    let mut source_maps = BTreeMap::new();
    source_maps.insert(
        PathBuf::from("main.rs"),
        RustAnalyzerSourceMap {
            uri: source_uri.to_owned(),
            text: source.text().to_owned(),
            map: generated.source_map.clone(),
        },
    );
    let requested_path = PathBuf::from("main.rs");
    let Some(project_root) = state.project_root.as_deref() else {
        return Some(RustAnalyzerWorkspace {
            files,
            source_maps,
            requested_path,
        });
    };
    if source.path().is_none() {
        return Some(RustAnalyzerWorkspace {
            files,
            source_maps,
            requested_path,
        });
    }
    let resolved = oxido_compiler::application::modules::resolve_project_modules(
        source,
        &generated.rust,
        LexOptions {
            project_mode: state.project_mode,
        },
        project_root,
    )
    .ok()?;
    files.insert(PathBuf::from("main.rs"), resolved.root_rust);
    for module in resolved.modules {
        if let Some(source_map) = module.source_map {
            let source_path = source_map
                .source_file
                .as_deref()
                .map(|relative| project_root.join(relative));
            if let Some(source_path) = source_path
                && let Ok(text) = std::fs::read_to_string(&source_path)
            {
                source_maps.insert(
                    module.path.clone(),
                    RustAnalyzerSourceMap {
                        uri: file_uri(&source_path),
                        text,
                        map: source_map,
                    },
                );
            }
        }
        files.insert(module.path, module.rust);
    }
    Some(RustAnalyzerWorkspace {
        files,
        source_maps,
        requested_path,
    })
}

fn position_at_generated(source: &str, generated: &str, position: &Value) -> Option<Option<Value>> {
    let offset = offset_at_position(source, position)?;
    let mut token_start = offset;
    while token_start > 0
        && source
            .as_bytes()
            .get(token_start - 1)
            .is_some_and(|byte| byte.is_ascii_alphanumeric() || *byte == b'_')
    {
        token_start -= 1;
    }
    let mut token_end = offset;
    while token_end < source.len()
        && source
            .as_bytes()
            .get(token_end)
            .is_some_and(|byte| byte.is_ascii_alphanumeric() || *byte == b'_')
    {
        token_end += 1;
    }
    let token = source.get(token_start..token_end)?;
    let occurrence = source
        .get(..offset)
        .unwrap_or_default()
        .match_indices(token)
        .count();
    let generated_offset = generated
        .match_indices(token)
        .nth(occurrence)
        .map(|(offset, _)| offset)
        .or_else(|| generated.find(token))?;
    Some(lsp_position(generated, generated_offset))
}

fn run_rust_analyzer(
    workspace: RustAnalyzerWorkspace,
    method: &str,
    id: &Value,
    params: Value,
) -> Option<Value> {
    let worker_lock = RA_WORKER.get_or_init(|| Mutex::new(None));
    let sender = {
        let mut guard = worker_lock.lock().ok()?;
        if guard.is_none() {
            let (sender, receiver) = mpsc::sync_channel::<RaWork>(4);
            thread::Builder::new()
                .name("oxido-lsp-rust-analyzer".to_owned())
                .spawn(move || rust_analyzer_worker(receiver))
                .ok()?;
            *guard = Some(sender);
        }
        guard.as_ref()?.clone()
    };
    let (reply_tx, reply_rx) = mpsc::channel();
    sender
        .try_send(RaWork {
            workspace,
            method: method.to_owned(),
            params,
            reply: reply_tx,
        })
        .ok()?;
    let mut response = reply_rx
        .recv_timeout(RUST_ANALYZER_RESPONSE_TIMEOUT)
        .ok()??;
    RA_RESPONSES.fetch_add(1, Ordering::Relaxed);
    if let Some(object) = response.as_object_mut() {
        object.insert("id".to_owned(), id.clone());
    }
    Some(response)
}

fn rust_analyzer_worker(receiver: mpsc::Receiver<RaWork>) {
    let mut session: Option<RustAnalyzerSession> = None;
    while let Ok(work) = receiver.recv() {
        if work.method == "__shutdown" {
            if work.reply.send(None).is_err() {
                eprintln!("el cliente LSP cerró la confirmación de cierre de rust-analyzer");
            }
            break;
        }
        let needs_restart = session
            .as_ref()
            .is_some_and(|item| item.files.keys().ne(work.workspace.files.keys()));
        if needs_restart && let Some(item) = session.take() {
            stop_rust_analyzer_session(item);
        }
        if session.is_none() {
            session = start_rust_analyzer(&work.workspace);
        }
        let result = session
            .as_mut()
            .and_then(|item| query_rust_analyzer(item, &work.workspace, &work.method, work.params));
        if work.reply.send(result).is_err() {
            eprintln!("el cliente LSP cerró la respuesta de rust-analyzer");
        }
    }
    if let Some(item) = session {
        stop_rust_analyzer_session(item);
    }
}

fn stop_rust_analyzer_session(mut item: RustAnalyzerSession) {
    if let Err(error) = item.child.kill() {
        eprintln!("no se pudo detener rust-analyzer: {error}");
    }
    if let Err(error) = item.child.wait() {
        eprintln!("no se pudo esperar rust-analyzer: {error}");
    }
    if let Err(error) = std::fs::remove_dir_all(item.root) {
        eprintln!("no se pudo limpiar workspace de rust-analyzer: {error}");
    }
}

fn query_rust_analyzer(
    session: &mut RustAnalyzerSession,
    workspace: &RustAnalyzerWorkspace,
    method: &str,
    params: Value,
) -> Option<Value> {
    for (path, rust) in &workspace.files {
        if session.files.get(path) == Some(rust) {
            continue;
        }
        let uri = format!("file://{}/src/{}", session.root.display(), path.display());
        session.next_id = session.next_id.saturating_add(1);
        {
            let mut input = session.input.lock().ok()?;
            write_lsp_message(&mut *input, &json!({"jsonrpc":"2.0","method":"textDocument/didChange","params":{"textDocument":{"uri":uri,"version":session.next_id},"contentChanges":[{"text":rust}]}})).ok()?;
        }
    }
    session.files = workspace.files.clone();
    let mut params = params;
    if let Some(document) = params.get_mut("textDocument") {
        *document = json!({"uri":format!("file://{}/src/{}", session.root.display(), workspace.requested_path.display())});
    }
    session.next_id = session.next_id.saturating_add(1);
    let request_id = session.next_id;
    let mut input = session.input.lock().ok()?;
    write_lsp_message(
        &mut *input,
        &json!({"jsonrpc":"2.0","id":request_id,"method":method,"params":params}),
    )
    .ok()?;
    drop(input);
    let mut response = wait_response(&session.output, request_id)?;
    map_rust_analyzer_locations(&mut response, workspace, &session.root);
    Some(response)
}

fn map_rust_analyzer_locations(
    value: &mut Value,
    workspace: &RustAnalyzerWorkspace,
    generated_root: &std::path::Path,
) {
    let Some(object) = value.as_object_mut() else {
        if let Some(values) = value.as_array_mut() {
            for value in values {
                map_rust_analyzer_locations(value, workspace, generated_root);
            }
        }
        return;
    };
    let mapped_path = object
        .get("uri")
        .and_then(Value::as_str)
        .and_then(file_uri_path)
        .map(PathBuf::from)
        .and_then(|path| {
            path.strip_prefix(generated_root.join("src"))
                .ok()
                .map(PathBuf::from)
        });
    if let Some(path) = mapped_path
        && let Some(source) = workspace.source_maps.get(&path)
    {
        object.insert("uri".to_owned(), Value::String(source.uri.clone()));
        if let Some(range) = object.get_mut("range") {
            map_rust_analyzer_range(range, source);
        }
        if let Some(range) = object.get_mut("targetRange") {
            map_rust_analyzer_range(range, source);
        }
        if let Some(range) = object.get_mut("targetSelectionRange") {
            map_rust_analyzer_range(range, source);
        }
    }
    let mapped_target_path = object
        .get("targetUri")
        .and_then(Value::as_str)
        .and_then(file_uri_path)
        .map(PathBuf::from)
        .and_then(|path| {
            path.strip_prefix(generated_root.join("src"))
                .ok()
                .map(PathBuf::from)
        });
    if let Some(path) = mapped_target_path
        && let Some(source) = workspace.source_maps.get(&path)
    {
        object.insert("targetUri".to_owned(), Value::String(source.uri.clone()));
        if let Some(range) = object.get_mut("targetRange") {
            map_rust_analyzer_range(range, source);
        }
        if let Some(range) = object.get_mut("targetSelectionRange") {
            map_rust_analyzer_range(range, source);
        }
    }
    for child in object.values_mut() {
        map_rust_analyzer_locations(child, workspace, generated_root);
    }
}

fn map_rust_analyzer_range(range: &mut Value, source: &RustAnalyzerSourceMap) {
    let Some(start) = range.get("start").cloned() else {
        return;
    };
    let Some(start) = map_rust_analyzer_position(&start, source) else {
        return;
    };
    let Some(end) = range
        .get("end")
        .and_then(|position| map_rust_analyzer_position(position, source))
    else {
        return;
    };
    *range = json!({"start":start,"end":end});
}

fn map_rust_analyzer_position(position: &Value, source: &RustAnalyzerSourceMap) -> Option<Value> {
    let line = usize::try_from(position.get("line")?.as_u64()?)
        .ok()?
        .checked_add(1)?;
    let column = usize::try_from(position.get("character")?.as_u64()?)
        .ok()?
        .checked_add(1)?;
    let location = source.map.map_generated(line, column)?;
    lsp_position(&source.text, location.start)
}

fn start_rust_analyzer(workspace: &RustAnalyzerWorkspace) -> Option<RustAnalyzerSession> {
    let root = std::env::temp_dir().join(format!(
        "oxido-ra-{}",
        NEXT_RA_WORKSPACE.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    ));
    let source_dir = root.join("src");
    std::fs::create_dir_all(&source_dir).ok()?;
    if std::fs::write(
        root.join("Cargo.toml"),
        "[package]\nname = \"oxido_lsp_generated\"\nversion = \"0.0.0\"\nedition = \"2024\"\n",
    )
    .is_err()
    {
        if let Err(error) = std::fs::remove_dir_all(&root) {
            eprintln!("no se pudo limpiar workspace de rust-analyzer: {error}");
        }
        return None;
    }
    for (path, rust) in &workspace.files {
        let destination = source_dir.join(path);
        if let Some(parent) = destination.parent()
            && std::fs::create_dir_all(parent).is_err()
        {
            if let Err(error) = std::fs::remove_dir_all(&root) {
                eprintln!("no se pudo limpiar workspace de rust-analyzer: {error}");
            }
            return None;
        }
        if std::fs::write(destination, rust).is_err() {
            if let Err(error) = std::fs::remove_dir_all(&root) {
                eprintln!("no se pudo limpiar workspace de rust-analyzer: {error}");
            }
            return None;
        }
    }
    let executable =
        std::env::var_os("OXIDO_RUST_ANALYZER").unwrap_or_else(|| "rust-analyzer".into());
    RA_STARTS.fetch_add(1, Ordering::Relaxed);
    let mut child = Command::new(executable)
        .current_dir(&root)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;
    let input = Arc::new(Mutex::new(child.stdin.take()?));
    let stdout = child.stdout.take()?;
    let (response_tx, response_rx) = mpsc::channel();
    thread::Builder::new()
        .name("oxido-lsp-rust-analyzer-reader".to_owned())
        .spawn({
            let reader_input = Arc::clone(&input);
            move || rust_analyzer_reader(stdout, response_tx, reader_input)
        })
        .ok()?;
    {
        let mut locked_input = input.lock().ok()?;
        write_lsp_message(&mut *locked_input, &json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{"processId":null,"rootUri":format!("file://{}", root.display()),"capabilities":{}}})).ok()?;
    }
    wait_response_with_timeout(&response_rx, 1, RUST_ANALYZER_START_TIMEOUT)?;
    {
        let mut locked_input = input.lock().ok()?;
        write_lsp_message(
            &mut *locked_input,
            &json!({"jsonrpc":"2.0","method":"initialized","params":{}}),
        )
        .ok()?;
    }
    {
        let mut locked_input = input.lock().ok()?;
        for (path, rust) in &workspace.files {
            let uri = format!("file://{}/src/{}", root.display(), path.display());
            write_lsp_message(&mut *locked_input, &json!({"jsonrpc":"2.0","method":"textDocument/didOpen","params":{"textDocument":{"uri":uri,"languageId":"rust","version":1,"text":rust}}})).ok()?;
        }
    }
    Some(RustAnalyzerSession {
        child,
        input,
        output: response_rx,
        root,
        files: workspace.files.clone(),
        next_id: 1,
    })
}

fn shutdown_rust_analyzer_worker() {
    let Some(worker_lock) = RA_WORKER.get() else {
        return;
    };
    let Ok(mut guard) = worker_lock.lock() else {
        return;
    };
    let Some(sender) = guard.take() else { return };
    let (reply_tx, reply_rx) = mpsc::channel();
    if let Err(error) = sender.send(RaWork {
        workspace: RustAnalyzerWorkspace {
            files: BTreeMap::new(),
            source_maps: BTreeMap::new(),
            requested_path: PathBuf::from("main.rs"),
        },
        method: "__shutdown".to_owned(),
        params: Value::Null,
        reply: reply_tx,
    }) {
        eprintln!("no se pudo cerrar worker de rust-analyzer: {error}");
        return;
    }
    if reply_rx.recv_timeout(Duration::from_secs(3)).is_err() {
        eprintln!("worker de rust-analyzer no confirmó el cierre");
    }
}

fn wait_response(input: &Receiver<Value>, expected_id: i64) -> Option<Value> {
    wait_response_with_timeout(input, expected_id, RUST_ANALYZER_RESPONSE_TIMEOUT)
}

fn wait_response_with_timeout(
    input: &Receiver<Value>,
    expected_id: i64,
    timeout: Duration,
) -> Option<Value> {
    let deadline = Instant::now().checked_add(timeout)?;
    loop {
        let remaining = deadline.checked_duration_since(Instant::now())?;
        let response = input.recv_timeout(remaining).ok()?;
        if response.get("id").and_then(Value::as_i64) == Some(expected_id) {
            return Some(response);
        }
    }
}

fn rust_analyzer_reader(stdout: ChildStdout, sender: Sender<Value>, input: Arc<Mutex<ChildStdin>>) {
    let mut output = BufReader::new(stdout);
    loop {
        let frame = match read_frame(&mut output) {
            Ok(Some(frame)) => frame,
            Ok(None) => break,
            Err(error) => {
                eprintln!("falló la lectura LSP de rust-analyzer: {error}");
                break;
            }
        };
        let message = match serde_json::from_slice::<Value>(&frame) {
            Ok(message) => message,
            Err(error) => {
                eprintln!("rust-analyzer envió JSON inválido: {error}");
                break;
            }
        };
        if message.get("method").is_some() {
            if let Some(id) = message.get("id") {
                let response = rust_analyzer_client_response(id, &message);
                let Ok(mut locked_input) = input.lock() else {
                    break;
                };
                if let Err(error) = write_lsp_message(&mut *locked_input, &response) {
                    eprintln!("no se pudo responder a rust-analyzer: {error}");
                    break;
                }
            }
            continue;
        }
        if message.get("id").is_none() {
            continue;
        }
        if sender.send(message).is_err() {
            break;
        }
    }
}

fn rust_analyzer_client_response(id: &Value, request: &Value) -> Value {
    let result = match request.get("method").and_then(Value::as_str) {
        Some("workspace/configuration") => request
            .pointer("/params/items")
            .and_then(Value::as_array)
            .map_or_else(
                || json!([]),
                |items| Value::Array(vec![Value::Null; items.len()]),
            ),
        _ => Value::Null,
    };
    json!({"jsonrpc":"2.0","id":id,"result":result})
}

fn workspace_documents(state: &ServerState) -> Vec<(String, String, Option<i64>, String)> {
    let mut documents = state
        .documents
        .iter()
        .map(|(uri, document)| {
            (
                uri.clone(),
                document.text.clone(),
                document.version,
                uri.clone(),
            )
        })
        .collect::<Vec<_>>();
    let Some(root) = state.project_root.as_deref() else {
        return documents;
    };
    let mut pending = vec![root.to_path_buf()];
    let mut scanned = 0_usize;
    while let Some(directory) = pending.pop() {
        let Ok(entries) = std::fs::read_dir(directory) else {
            continue;
        };
        for entry in entries.flatten() {
            scanned = scanned.saturating_add(1);
            if scanned > 512 {
                return documents;
            }
            let path = entry.path();
            let Ok(metadata) = std::fs::symlink_metadata(&path) else {
                continue;
            };
            if metadata.file_type().is_symlink() {
                continue;
            }
            if metadata.is_dir() {
                if path.file_name().is_some_and(|name| {
                    matches!(name.to_str(), Some("target" | ".git" | "node_modules"))
                }) {
                    continue;
                }
                pending.push(path);
                continue;
            }
            if !metadata.is_file()
                || path.extension().and_then(|extension| extension.to_str()) != Some("ox")
                || metadata.len() > MAX_SINGLE_DOCUMENT_BYTES as u64
            {
                continue;
            }
            let Ok(canonical) = path.canonicalize() else {
                continue;
            };
            if !canonical.starts_with(root) {
                continue;
            }
            let uri = file_uri(&canonical);
            if state.documents.contains_key(&uri)
                || documents.iter().any(|(known, _, _, _)| known == &uri)
            {
                continue;
            }
            let Ok(text) = std::fs::read_to_string(&canonical) else {
                continue;
            };
            documents.push((uri.clone(), text, None, uri));
            if documents.len() >= MAX_OPEN_DOCUMENTS {
                return documents;
            }
        }
    }
    documents
}

fn file_uri(path: &std::path::Path) -> String {
    format!("file://{}", path.to_string_lossy().replace(' ', "%20"))
}

fn workspace_symbols(state: &ServerState) -> Vec<WorkspaceSymbol> {
    let mut symbols = Vec::new();
    for (uri, text, _, _) in workspace_documents(state) {
        let module_path = file_module_path(&uri, state.project_root.as_deref());
        let declarations = declared_symbols(&text, state.project_mode);
        if declarations.is_empty() {
            for (name, selection, range) in textual_function_declarations(&text) {
                symbols.push(WorkspaceSymbol {
                    uri: uri.clone(),
                    name,
                    kind: SymbolKind::Function,
                    container: None,
                    module_path: module_path.clone(),
                    selection,
                    range,
                });
            }
            continue;
        }
        for declaration in declarations {
            let (Some(selection_start), Some(selection_end), Some(range_start), Some(range_end)) = (
                lsp_position(&text, declaration.name_start),
                lsp_position(&text, declaration.name_end),
                lsp_position(&text, declaration.range_start),
                lsp_position(&text, declaration.range_end),
            ) else {
                continue;
            };
            let mut nested_path = module_path.clone();
            nested_path.extend(declaration.module_path);
            symbols.push(WorkspaceSymbol {
                uri: uri.clone(),
                name: declaration.name,
                kind: declaration.kind,
                container: declaration.container,
                module_path: nested_path,
                selection: json!({"start":selection_start,"end":selection_end}),
                range: json!({"start":range_start,"end":range_end}),
            });
        }
    }
    symbols
}

fn file_module_path(uri: &str, root: Option<&std::path::Path>) -> Vec<String> {
    let Some(path) = uri.strip_prefix("file://") else {
        return Vec::new();
    };
    let path = PathBuf::from(path.replace("%20", " "));
    let relative = root
        .and_then(|root| path.strip_prefix(root).ok())
        .unwrap_or(path.as_path());
    relative
        .components()
        .filter_map(|component| match component {
            std::path::Component::Normal(value) => value.to_str(),
            _ => None,
        })
        .map(|value| {
            value
                .strip_suffix(".ox")
                .map_or_else(|| value.to_owned(), str::to_owned)
        })
        .collect()
}

fn textual_function_declarations(text: &str) -> Vec<(String, Value, Value)> {
    let mut declarations = Vec::new();
    let mut offset = 0_usize;
    for line in text.split_inclusive('\n') {
        let trimmed = line.trim_start();
        let declaration = trimmed.strip_prefix("pub ").unwrap_or(trimmed);
        let Some(rest) = declaration
            .strip_prefix("funcion ")
            .or_else(|| declaration.strip_prefix("fn "))
        else {
            offset = offset.saturating_add(line.len());
            continue;
        };
        let Some(name) = rest
            .split(|character: char| !(character.is_ascii_alphanumeric() || character == '_'))
            .next()
            .filter(|name| !name.is_empty())
        else {
            offset = offset.saturating_add(line.len());
            continue;
        };
        let start = offset + line.len() - trimmed.len()
            + (trimmed.len() - declaration.len())
            + if declaration.starts_with("funcion ") {
                8
            } else {
                3
            };
        let end = start + name.len();
        let range_end = offset + line.trim_end_matches(['\n', '\r']).len();
        let Some(selection_start) = lsp_position(text, start) else {
            continue;
        };
        let Some(selection_end) = lsp_position(text, end) else {
            continue;
        };
        let Some(range_start) = lsp_position(text, offset) else {
            continue;
        };
        let Some(range_end) = lsp_position(text, range_end) else {
            continue;
        };
        declarations.push((
            name.to_owned(),
            json!({"start":selection_start,"end":selection_end}),
            json!({"start":range_start,"end":range_end}),
        ));
        offset = offset.saturating_add(line.len());
    }
    declarations
}

fn request_document_position<'a>(
    params: Option<&Value>,
    state: &'a ServerState,
) -> Option<(String, &'a str, usize)> {
    let uri = params?.pointer("/textDocument/uri")?.as_str()?.to_owned();
    let document = state.documents.get(&uri)?;
    let offset = offset_at_position(&document.text, params?.get("position")?)?;
    Some((uri, &document.text, offset))
}

fn lex_token_at(
    text: &str,
    offset: usize,
    project_mode: Option<oxido_language::domain::syntax::SyntaxMode>,
) -> Option<oxido_language::domain::syntax::Token> {
    let lexed = lex(&SourceFile::new(text), LexOptions { project_mode });
    lexed
        .tokens
        .into_iter()
        .find(|token| token.span.start() <= offset && offset < token.span.end())
}

fn open_document(request: &Value, state: &mut ServerState) -> Vec<Value> {
    let Some(document) = request.pointer("/params/textDocument") else {
        return Vec::new();
    };
    let (Some(uri), Some(text)) = (
        document.get("uri").and_then(Value::as_str),
        document.get("text").and_then(Value::as_str),
    ) else {
        return Vec::new();
    };
    let uri = uri.to_owned();
    let text = text.to_owned();
    let previous_size = state.documents.get(&uri).map_or(0, |item| item.text.len());
    let projected_total = state
        .total_document_bytes
        .saturating_sub(previous_size)
        .saturating_add(text.len());
    let issue = if text.len() > MAX_SINGLE_DOCUMENT_BYTES {
        Some(("OX6001", "el documento supera el límite de 4 MiB"))
    } else if !state.documents.contains_key(&uri) && state.documents.len() >= MAX_OPEN_DOCUMENTS {
        Some(("OX6002", "se alcanzó el límite de 64 documentos abiertos"))
    } else if projected_total > MAX_OPEN_DOCUMENT_BYTES {
        Some((
            "OX6003",
            "los documentos abiertos superan el límite total de 16 MiB",
        ))
    } else {
        None
    };
    if let Some((code, message)) = issue {
        return vec![resource_diagnostic(&uri, code, message)];
    }
    state.total_document_bytes = projected_total;
    state.documents.insert(
        uri,
        TextDocument {
            version: document.get("version").and_then(Value::as_i64),
            text,
        },
    );
    Vec::new()
}

fn change_document(request: &Value, state: &mut ServerState) -> Vec<Value> {
    let Some(uri) = request
        .pointer("/params/textDocument/uri")
        .and_then(Value::as_str)
    else {
        return Vec::new();
    };
    let Some(document) = state.documents.get(uri) else {
        return Vec::new();
    };
    let incoming_version = request
        .pointer("/params/textDocument/version")
        .and_then(Value::as_i64);
    if let (Some(current), Some(incoming)) = (document.version, incoming_version)
        && incoming <= current
    {
        return Vec::new();
    }
    let Some(changes) = request
        .pointer("/params/contentChanges")
        .and_then(Value::as_array)
    else {
        return Vec::new();
    };
    let Some(updated_text) = apply_content_changes(&document.text, changes) else {
        return Vec::new();
    };
    if updated_text.len() > MAX_SINGLE_DOCUMENT_BYTES {
        return vec![resource_diagnostic(
            uri,
            "OX6001",
            "el documento supera el límite de 4 MiB",
        )];
    }
    let projected_total = state
        .total_document_bytes
        .saturating_sub(document.text.len())
        .saturating_add(updated_text.len());
    if projected_total > MAX_OPEN_DOCUMENT_BYTES {
        return vec![resource_diagnostic(
            uri,
            "OX6003",
            "los documentos abiertos superan el límite total de 16 MiB",
        )];
    }
    if let Some(document) = state.documents.get_mut(uri) {
        state.total_document_bytes = projected_total;
        document.text = updated_text;
        if incoming_version.is_some() {
            document.version = incoming_version;
        }
    }
    Vec::new()
}

fn close_document(request: &Value, state: &mut ServerState) {
    let Some(uri) = request
        .pointer("/params/textDocument/uri")
        .and_then(Value::as_str)
    else {
        return;
    };
    if let Some(document) = state.documents.remove(uri) {
        state.total_document_bytes = state
            .total_document_bytes
            .saturating_sub(document.text.len());
    }
}

fn resource_diagnostic(uri: &str, code: &str, message: &str) -> Value {
    json!({"jsonrpc":"2.0","method":"textDocument/publishDiagnostics","params":{"uri":uri,"diagnostics":[{"range":{"start":{"line":0,"character":0},"end":{"line":0,"character":0}},"severity":1,"code":code,"source":"oxido-lsp","message":message}]}})
}

fn apply_content_changes(current: &str, changes: &[Value]) -> Option<String> {
    let mut updated = current.to_owned();
    for change in changes {
        let replacement = change.get("text")?.as_str()?;
        let Some(range) = change.get("range") else {
            updated = replacement.to_owned();
            continue;
        };
        let start = offset_at_position(&updated, range.get("start")?)?;
        let end = offset_at_position(&updated, range.get("end")?)?;
        if start > end {
            return None;
        }
        updated.replace_range(start..end, replacement);
    }
    Some(updated)
}

fn offset_at_position(text: &str, position: &Value) -> Option<usize> {
    let target_line = usize::try_from(position.get("line")?.as_u64()?).ok()?;
    let target_character = usize::try_from(position.get("character")?.as_u64()?).ok()?;
    let mut line_start = 0_usize;
    for (line_number, raw_line) in text.split('\n').enumerate() {
        if line_number == target_line {
            let line = raw_line.strip_suffix('\r').unwrap_or(raw_line);
            let mut utf16_column = 0_usize;
            for (byte_column, character) in line.char_indices() {
                if utf16_column == target_character {
                    return Some(line_start + byte_column);
                }
                utf16_column = utf16_column.checked_add(character.len_utf16())?;
                if utf16_column > target_character {
                    return None;
                }
            }
            return (utf16_column == target_character).then_some(line_start + line.len());
        }
        line_start = line_start.checked_add(raw_line.len())?.checked_add(1)?;
    }
    None
}

fn semantic_response(id: Value, params: Option<&Value>, state: &ServerState) -> Value {
    let uri = params
        .and_then(|value| value.pointer("/textDocument/uri"))
        .and_then(Value::as_str);
    let data = uri
        .and_then(|uri| state.documents.get(uri))
        .map(|document| semantic_tokens(&document.text, state.project_mode))
        .unwrap_or_default();
    json!({"jsonrpc":"2.0","id":id,"result":{"data":data}})
}

fn semantic_tokens(
    text: &str,
    project_mode: Option<oxido_language::domain::syntax::SyntaxMode>,
) -> Vec<u32> {
    let cancellation = AtomicBool::new(false);
    semantic_tokens_cancellable(text, project_mode, &cancellation).unwrap_or_default()
}

fn semantic_tokens_cancellable(
    text: &str,
    project_mode: Option<oxido_language::domain::syntax::SyntaxMode>,
    cancellation: &AtomicBool,
) -> Option<Vec<u32>> {
    let source = SourceFile::new(text);
    let lexed = lex(&source, LexOptions { project_mode });
    let mut semantic = Vec::<(usize, usize, usize, usize)>::new();
    let mut previous_significant = None;
    for (index, token) in lexed.tokens.iter().enumerate() {
        if cancellation.load(Ordering::Relaxed) {
            return None;
        }
        let is_trivia = matches!(
            &token.kind,
            TokenKind::Comment
                | TokenKind::Newline
                | TokenKind::Indent
                | TokenKind::Dedent
                | TokenKind::ModeHeader(_)
        );
        if token.opaque {
            continue;
        }
        let Some(kind) = semantic_kind(
            token,
            previous_significant,
            lexed.tokens.get(index + 1),
            &source,
        ) else {
            continue;
        };
        let Some(start) = lsp_position(text, token.span.start()) else {
            continue;
        };
        let Some(end) = lsp_position(text, token.span.end()) else {
            continue;
        };
        if start["line"] != end["line"] {
            continue;
        }
        let line = usize::try_from(start["line"].as_u64()?).ok()?;
        let character = usize::try_from(start["character"].as_u64()?).ok()?;
        let end_character = usize::try_from(end["character"].as_u64()?).ok()?;
        let length = end_character.checked_sub(character)?;
        if length > 0 {
            semantic.push((line, character, length, kind));
        }
        if !is_trivia {
            previous_significant = Some(token);
        }
    }
    semantic.sort_unstable();
    let mut data = Vec::with_capacity(semantic.len().saturating_mul(5));
    let (mut previous_line, mut previous_character) = (0_usize, 0_usize);
    for (line, character, length, kind) in semantic {
        let delta_line = line.checked_sub(previous_line)?;
        let delta_character = if delta_line == 0 {
            character.checked_sub(previous_character)?
        } else {
            character
        };
        data.extend([
            u32::try_from(delta_line).ok()?,
            u32::try_from(delta_character).ok()?,
            u32::try_from(length).ok()?,
            u32::try_from(kind).ok()?,
            0,
        ]);
        previous_line = line;
        previous_character = character;
    }
    (!cancellation.load(Ordering::Relaxed)).then_some(data)
}

fn project_syntax_mode(request: &Value) -> Option<oxido_language::domain::syntax::SyntaxMode> {
    if let Some(mode) = request
        .pointer("/params/initializationOptions/syntaxMode")
        .and_then(Value::as_str)
    {
        return parse_syntax_mode(mode);
    }
    let root_uri = request.pointer("/params/rootUri").and_then(Value::as_str)?;
    project_mode_for_uri(root_uri)
}

fn parse_syntax_mode(mode: &str) -> Option<oxido_language::domain::syntax::SyntaxMode> {
    match mode {
        "indentation" => Some(oxido_language::domain::syntax::SyntaxMode::Indentation),
        "braces" => Some(oxido_language::domain::syntax::SyntaxMode::Braces),
        _ => None,
    }
}

fn project_mode_for_uri(uri: &str) -> Option<oxido_language::domain::syntax::SyntaxMode> {
    let root = PathBuf::from(file_uri_path(uri)?);
    let root = std::fs::canonicalize(root).ok()?;
    if !root.is_dir() {
        return None;
    }
    let config_path = root.join("oxido.toml");
    let canonical_config = std::fs::canonicalize(&config_path).ok()?;
    if canonical_config.parent()? != root
        || std::fs::metadata(&canonical_config).ok()?.len() > 64 * 1024
    {
        return None;
    }
    let contents = std::fs::read_to_string(canonical_config).ok()?;
    let config = oxido_compiler::config::ProjectConfig::parse(&contents).ok()?;
    config.language.syntax.map(|mode| match mode {
        oxido_compiler::config::SyntaxMode::Indentation => {
            oxido_language::domain::syntax::SyntaxMode::Indentation
        }
        oxido_compiler::config::SyntaxMode::Braces => {
            oxido_language::domain::syntax::SyntaxMode::Braces
        }
    })
}

fn file_uri_path(uri: &str) -> Option<String> {
    let encoded = uri.strip_prefix("file://")?;
    if !encoded.starts_with('/') {
        return None;
    }
    let mut bytes = Vec::with_capacity(encoded.len());
    let input = encoded.as_bytes();
    let mut index = 0;
    while index < input.len() {
        if input[index] == b'%' {
            let high = *input.get(index + 1)?;
            let low = *input.get(index + 2)?;
            bytes.push((hex_value(high)? << 4) | hex_value(low)?);
            index += 3;
        } else {
            bytes.push(input[index]);
            index += 1;
        }
    }
    String::from_utf8(bytes).ok()
}

fn hex_value(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

fn semantic_kind(
    token: &oxido_language::domain::syntax::Token,
    previous: Option<&oxido_language::domain::syntax::Token>,
    next: Option<&oxido_language::domain::syntax::Token>,
    source: &SourceFile,
) -> Option<usize> {
    match &token.kind {
        TokenKind::Keyword { .. } => Some(0),
        TokenKind::StringLiteral | TokenKind::CharacterLiteral => Some(3),
        TokenKind::Number => Some(4),
        TokenKind::Comment => Some(5),
        TokenKind::Punctuation => Some(7),
        TokenKind::Identifier { .. } => {
            let word = source.slice(token.span);
            if let Some(oxido_language::domain::syntax::Token {
                kind: TokenKind::Keyword { rust_keyword, .. },
                ..
            }) = previous
            {
                if *rust_keyword == "fn" {
                    return Some(1);
                }
                if matches!(*rust_keyword, "struct" | "enum" | "trait" | "type") {
                    return Some(6);
                }
            }
            let next_text = next.map(|token| source.slice(token.span));
            if next_text == Some("(") {
                Some(1)
            } else if word.chars().next().is_some_and(char::is_uppercase) {
                Some(6)
            } else {
                Some(2)
            }
        }
        _ => None,
    }
}

fn lsp_position(text: &str, offset: usize) -> Option<Value> {
    if offset > text.len() || !text.is_char_boundary(offset) {
        return None;
    }
    let prefix = &text[..offset];
    let line = prefix.bytes().filter(|byte| *byte == b'\n').count();
    let line_start = prefix.rfind('\n').map_or(0, |index| index + 1);
    let character = text[line_start..offset].encode_utf16().count();
    Some(json!({"line":line,"character":character}))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn configured_test_state() -> ServerState {
        ServerState {
            project_mode: Some(oxido_language::domain::syntax::SyntaxMode::Indentation),
            ..ServerState::default()
        }
    }

    #[test]
    fn handles_lifecycle_and_rejects_malformed_json_without_panicking() {
        let mut state = configured_test_state();
        let result = handle_json(
            br#"{"jsonrpc":"2.0","id":1,"method":"initialize"}"#,
            &mut state,
        )
        .expect("valid initialize request");
        assert_eq!(
            result[0]["result"]["capabilities"]["textDocumentSync"]["change"],
            2
        );
        assert!(handle_json(b"{", &mut state).is_err());
    }

    #[test]
    fn initialize_uses_explicit_project_syntax_mode() {
        let mut state = ServerState::default();
        handle_json(
            br#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"initializationOptions":{"syntaxMode":"braces"}}}"#,
            &mut state,
        )
        .expect("valid initialize request");
        assert_eq!(
            state.project_mode,
            Some(oxido_language::domain::syntax::SyntaxMode::Braces)
        );
    }

    #[test]
    fn initialize_reads_the_project_mode_as_data_from_the_workspace_root() {
        let mut state = ServerState::default();
        let project_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../prueba/modulos_jerarquicos")
            .canonicalize()
            .expect("reference project root exists");
        let request = json!({
            "jsonrpc":"2.0",
            "id":1,
            "method":"initialize",
            "params":{"rootUri":format!("file://{}", project_root.display())}
        });
        let encoded = serde_json::to_vec(&request).expect("initialize JSON serializes");
        handle_json(&encoded, &mut state).expect("valid initialize request");
        assert_eq!(
            state.project_mode,
            Some(oxido_language::domain::syntax::SyntaxMode::Braces)
        );
    }

    #[test]
    fn maps_compiler_spans_to_utf16_and_keeps_the_original_ox_uri() {
        let mut state = configured_test_state();
        let request = json!({
            "jsonrpc":"2.0",
            "method":"textDocument/didOpen",
            "params":{"textDocument":{"uri":"file:///workspace/principal.ox","text":format!("// 😀{}\n", '\u{202e}')}}
        });
        let encoded = serde_json::to_vec(&request).expect("didOpen JSON serializes");
        let response = handle_json(&encoded, &mut state).expect("valid didOpen notification");
        assert_eq!(
            response[0]["params"]["uri"],
            "file:///workspace/principal.ox"
        );
        let diagnostic = &response[0]["params"]["diagnostics"][0];
        assert_eq!(diagnostic["code"], "OX1002");
        assert_eq!(diagnostic["range"]["start"]["line"], 0);
        assert_eq!(diagnostic["range"]["start"]["character"], 5);
    }

    #[test]
    fn publishes_frontend_diagnostics_after_document_open() {
        let mut state = configured_test_state();
        let response = handle_json(
            br#"{"jsonrpc":"2.0","method":"textDocument/didOpen","params":{"textDocument":{"uri":"file:///main.ox","text":"funcion main():\n\tpass\n"}}}"#,
            &mut state,
        )
        .expect("valid didOpen request");
        assert_eq!(response.len(), 1);
        assert_eq!(response[0]["method"], "textDocument/publishDiagnostics");
        assert!(
            !response[0]["params"]["diagnostics"]
                .as_array()
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn enforces_open_document_and_total_memory_limits() {
        let mut state = ServerState::default();
        for index in 0..MAX_OPEN_DOCUMENTS {
            let request = json!({
                "jsonrpc":"2.0", "method":"textDocument/didOpen",
                "params":{"textDocument":{"uri":format!("file:///doc-{index}.ox"),"text":""}}
            });
            assert!(handle(request, &mut state).is_empty());
        }
        let rejected = handle(
            json!({
                "jsonrpc":"2.0", "method":"textDocument/didOpen",
                "params":{"textDocument":{"uri":"file:///overflow.ox","text":""}}
            }),
            &mut state,
        );
        assert_eq!(rejected[0]["params"]["diagnostics"][0]["code"], "OX6002");
        assert_eq!(state.documents.len(), MAX_OPEN_DOCUMENTS);

        let mut memory_state = ServerState::default();
        let document_text = "a".repeat(MAX_SINGLE_DOCUMENT_BYTES);
        for index in 0..4 {
            handle(
                json!({
                    "jsonrpc":"2.0", "method":"textDocument/didOpen",
                    "params":{"textDocument":{"uri":format!("file:///accepted-{index}.ox"),"text":document_text.clone()}}
                }),
                &mut memory_state,
            );
        }
        let rejected = handle(
            json!({
                "jsonrpc":"2.0", "method":"textDocument/didOpen",
                "params":{"textDocument":{"uri":"file:///over-budget.ox","text":"x"}}
            }),
            &mut memory_state,
        );
        assert_eq!(rejected[0]["params"]["diagnostics"][0]["code"], "OX6003");
        assert_eq!(memory_state.documents.len(), 4);
        assert_eq!(memory_state.total_document_bytes, MAX_OPEN_DOCUMENT_BYTES);
    }

    #[test]
    fn closing_a_document_releases_its_memory_budget() {
        let mut state = ServerState::default();
        handle(
            json!({
                "jsonrpc":"2.0", "method":"textDocument/didOpen",
                "params":{"textDocument":{"uri":"file:///main.ox","text":"contenido"}}
            }),
            &mut state,
        );
        assert_eq!(state.total_document_bytes, "contenido".len());
        handle(
            json!({
                "jsonrpc":"2.0", "method":"textDocument/didClose",
                "params":{"textDocument":{"uri":"file:///main.ox"}}
            }),
            &mut state,
        );
        assert!(state.documents.is_empty());
        assert_eq!(state.total_document_bytes, 0);
    }

    #[test]
    fn diagnostics_are_queued_from_a_document_snapshot_and_version() {
        let mut state = configured_test_state();
        let opened = handle(
            json!({
                "jsonrpc":"2.0", "method":"textDocument/didOpen",
                "params":{"textDocument":{"uri":"file:///main.ox","version":7,"text":"funcion main():\n\tpass\n"}}
            }),
            &mut state,
        );
        assert!(opened.is_empty());
        let (sender, receiver) = mpsc::sync_channel(1);
        let mut dirty = BTreeSet::from(["file:///main.ox".to_owned()]);
        let mut pending = HashMap::new();
        dispatch_diagnostic(&sender, &state, &mut dirty, &mut pending);
        let task = receiver.try_recv().expect("diagnostic task was queued");
        assert_eq!(task.version, Some(7));
        assert!(task.text.contains("pass"));
        assert!(
            !pending["file:///main.ox"]
                .cancellation
                .load(Ordering::Relaxed)
        );
        assert!(dirty.is_empty());
    }

    #[test]
    fn recovers_diagnostics_after_a_full_document_change() {
        let mut state = configured_test_state();
        let opened = handle_json(
            br#"{"jsonrpc":"2.0","method":"textDocument/didOpen","params":{"textDocument":{"uri":"file:///main.ox","text":"funcion main():\n\tdejar valor = 1\n"}}}"#,
            &mut state,
        )
        .expect("valid didOpen notification");
        assert!(
            !opened[0]["params"]["diagnostics"]
                .as_array()
                .expect("diagnostics are an array")
                .is_empty()
        );

        let changed = handle_json(
            br#"{"jsonrpc":"2.0","method":"textDocument/didChange","params":{"textDocument":{"uri":"file:///main.ox","version":2},"contentChanges":[{"text":"funcion main():\n    dejar valor = 1\n"}]}}"#,
            &mut state,
        )
        .expect("valid full-document didChange notification");
        assert_eq!(changed.len(), 1);
        assert_eq!(changed[0]["method"], "textDocument/publishDiagnostics");
        assert!(
            changed[0]["params"]["diagnostics"]
                .as_array()
                .expect("diagnostics are an array")
                .is_empty()
        );
    }

    #[test]
    fn applies_sequential_incremental_changes_using_utf16_positions() {
        let mut state = configured_test_state();
        handle_json(
            br#"{"jsonrpc":"2.0","method":"textDocument/didOpen","params":{"textDocument":{"uri":"file:///main.ox","version":1,"text":"funcion main():\n\tdejar valor = 1\n"}}}"#,
            &mut state,
        )
        .expect("valid didOpen notification");
        let changed = handle_json(
            br#"{"jsonrpc":"2.0","method":"textDocument/didChange","params":{"textDocument":{"uri":"file:///main.ox","version":2},"contentChanges":[{"range":{"start":{"line":1,"character":0},"end":{"line":1,"character":1}},"text":"    "},{"range":{"start":{"line":1,"character":18},"end":{"line":1,"character":19}},"text":"2"}]}}"#,
            &mut state,
        )
        .expect("valid incremental didChange notification");
        assert_eq!(
            state.documents["file:///main.ox"].text,
            "funcion main():\n    dejar valor = 2\n"
        );
        assert!(
            changed[0]["params"]["diagnostics"]
                .as_array()
                .expect("diagnostics are an array")
                .is_empty()
        );
    }

    #[test]
    fn rejects_invalid_incremental_ranges_without_corrupting_the_document() {
        let mut state = configured_test_state();
        handle_json(
            r#"{"jsonrpc":"2.0","method":"textDocument/didOpen","params":{"textDocument":{"uri":"file:///main.ox","version":1,"text":"// 😀x\nfuncion main():\n    dejar valor = 1\n"}}}"#.as_bytes(),
            &mut state,
        )
        .expect("valid didOpen notification");
        assert_eq!(
            offset_at_position("😀x\n", &json!({"line":0,"character":1})),
            None
        );

        let original = state.documents["file:///main.ox"].text.clone();
        handle_json(
            br#"{"jsonrpc":"2.0","method":"textDocument/didChange","params":{"textDocument":{"uri":"file:///main.ox","version":2},"contentChanges":[{"range":{"start":{"line":0,"character":4},"end":{"line":0,"character":5}},"text":"x"}]}}"#,
            &mut state,
        )
        .expect("well-formed notification with invalid UTF-16 range");
        assert_eq!(state.documents["file:///main.ox"].text, original);
    }

    #[test]
    fn ignores_stale_document_versions() {
        let mut state = configured_test_state();
        handle_json(
            br#"{"jsonrpc":"2.0","method":"textDocument/didOpen","params":{"textDocument":{"uri":"file:///main.ox","version":4,"text":"funcion main():\n    dejar valor = 1\n"}}}"#,
            &mut state,
        )
        .expect("valid didOpen notification");
        handle_json(
            br#"{"jsonrpc":"2.0","method":"textDocument/didChange","params":{"textDocument":{"uri":"file:///main.ox","version":3},"contentChanges":[{"text":"texto obsoleto"}]}}"#,
            &mut state,
        )
        .expect("well-formed stale notification");
        assert_eq!(
            state.documents["file:///main.ox"].text,
            "funcion main():\n    dejar valor = 1\n"
        );
        assert_eq!(state.documents["file:///main.ox"].version, Some(4));
    }

    #[test]
    fn returns_semantic_tokens_for_an_open_document() {
        let mut state = configured_test_state();
        handle_json(
            br#"{"jsonrpc":"2.0","method":"textDocument/didOpen","params":{"textDocument":{"uri":"file:///main.ox","text":"funcion main():\n    pass\n"}}}"#,
            &mut state,
        )
        .expect("valid didOpen request");
        let response = handle_json(
            br#"{"jsonrpc":"2.0","id":2,"method":"textDocument/semanticTokens/full","params":{"textDocument":{"uri":"file:///main.ox"}}}"#,
            &mut state,
        )
        .expect("valid semantic tokens request");
        assert_eq!(response[0]["result"]["data"][0], 0);
        assert_eq!(response[0]["result"]["data"][1], 0);
        assert_eq!(response[0]["result"]["data"][2], 7);
        assert_eq!(response[0]["result"]["data"][3], 0);
    }

    #[test]
    fn provides_syntax_based_hover_completion_symbols_and_navigation() {
        let mut state = configured_test_state();
        let text = "sintaxis indentacion\nfuncion saludo():\n    imprimir!(\"hola\")\nfuncion principal():\n    saludo()\n";
        handle(
            json!({
                "jsonrpc":"2.0", "method":"textDocument/didOpen",
                "params":{"textDocument":{"uri":"file:///main.ox","version":1,"text":text}}
            }),
            &mut state,
        );

        let hover = handle(
            json!({
                "jsonrpc":"2.0", "id":2, "method":"textDocument/hover",
                "params":{"textDocument":{"uri":"file:///main.ox"},"position":{"line":1,"character":1}}
            }),
            &mut state,
        );
        assert!(
            hover[0]["result"]["contents"]["value"]
                .as_str()
                .unwrap()
                .contains("`funcion`")
        );

        let completion = handle(
            json!({
                "jsonrpc":"2.0", "id":3, "method":"textDocument/completion",
                "params":{"textDocument":{"uri":"file:///main.ox"},"position":{"line":4,"character":10}}
            }),
            &mut state,
        );
        assert!(
            completion[0]["result"]["items"]
                .as_array()
                .unwrap()
                .iter()
                .any(|item| item["label"] == "saludo")
        );

        let symbols = handle(
            json!({"jsonrpc":"2.0","id":4,"method":"textDocument/documentSymbol","params":{"textDocument":{"uri":"file:///main.ox"}}}),
            &mut state,
        );
        assert!(!symbols[0]["result"].as_array().unwrap().is_empty());
        assert_eq!(symbols[0]["result"][0]["name"], "saludo");

        let definition = handle(
            json!({
                "jsonrpc":"2.0", "id":5, "method":"textDocument/definition",
                "params":{"textDocument":{"uri":"file:///main.ox"},"position":{"line":4,"character":6}}
            }),
            &mut state,
        );
        assert_eq!(definition[0]["result"][0]["uri"], "file:///main.ox");
        assert_eq!(definition[0]["result"][0]["range"]["start"]["line"], 1);

        let references = handle(
            json!({
                "jsonrpc":"2.0", "id":6, "method":"textDocument/references",
                "params":{"textDocument":{"uri":"file:///main.ox"},"position":{"line":4,"character":6}}
            }),
            &mut state,
        );
        assert_eq!(references[0]["result"].as_array().unwrap().len(), 2);
    }

    #[test]
    fn cargo_check_requires_explicit_workspace_trust() {
        let state = ServerState::default();
        let response = cargo_check_snapshot(
            &json!({
                "jsonrpc":"2.0", "id":9, "method":"oxido/check",
                "params":{"trusted":false,"textDocument":{"uri":"file:///main.ox"}}
            }),
            &state,
        )
        .expect_err("untrusted Cargo request is rejected");
        assert_eq!(response["error"]["code"], -32003);
    }

    #[test]
    fn maps_cargo_diagnostics_to_their_sources_and_utf16_columns() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../editors/vscode-oxido/test/fixtures/cargo")
            .canonicalize()
            .expect("cargo fixture root exists");
        let source_text =
            std::fs::read_to_string(root.join("util_error.ox")).expect("fixture is readable");
        let start = source_text
            .find("\"texto\"")
            .expect("fixture has the error literal");
        let end = start + "\"texto\"".len();
        let diagnostic = oxido_compiler::application::cargo::MappedCargoDiagnostic {
            level: "error".to_owned(),
            code: Some("E0308".to_owned()),
            message: "tipos incompatibles".to_owned(),
            source_file: Some("util_error.ox".to_owned()),
            source: Some(oxido_compiler::application::source_map::SourceLocation {
                line: 4,
                column: 25,
                start,
                end,
            }),
        };
        let state = ServerState {
            project_root: Some(root),
            ..ServerState::default()
        };
        let files = cargo_diagnostics_by_uri(
            &[diagnostic],
            false,
            &state,
            "file:///check_error.ox",
            "fallback",
        );
        assert_eq!(files.len(), 1);
        assert!(
            files[0]["uri"]
                .as_str()
                .is_some_and(|uri| uri.ends_with("/util_error.ox"))
        );
        assert_eq!(
            files[0]["diagnostics"][0]["range"]["start"],
            json!({"line": 3, "character": 24})
        );
        assert_eq!(files[0]["diagnostics"][0]["code"], "E0308");
    }

    #[test]
    fn cargo_mapping_preserves_utf16_columns_and_omits_failure_notes() {
        let root =
            std::env::temp_dir().join(format!("oxido-lsp-cargo-mapping-{}", std::process::id()));
        std::fs::create_dir_all(&root).expect("temporary fixture directory is created");
        let source_text = "sintaxis indentacion\nfuncion f():\n    dejar ñ = \"😀\"; dejar numero: i32 = \"texto\"\n";
        std::fs::write(root.join("modulo.ox"), source_text).expect("temporary source is written");
        let start = source_text
            .find("\"texto\"")
            .expect("temporary source has the error literal");
        let line_start = source_text[..start]
            .rfind('\n')
            .map_or(0, |index| index + 1);
        let expected_character = source_text[line_start..start].encode_utf16().count();
        let diagnostic = oxido_compiler::application::cargo::MappedCargoDiagnostic {
            level: "error".to_owned(),
            code: Some("E0308".to_owned()),
            message: "tipos incompatibles".to_owned(),
            source_file: Some("modulo.ox".to_owned()),
            source: Some(oxido_compiler::application::source_map::SourceLocation {
                line: 3,
                column: 1,
                start,
                end: start + "\"texto\"".len(),
            }),
        };
        let failure_note = oxido_compiler::application::cargo::MappedCargoDiagnostic {
            level: "failure-note".to_owned(),
            code: Some("E0308".to_owned()),
            message: "nota sin ubicación".to_owned(),
            source_file: None,
            source: None,
        };
        let state = ServerState {
            project_root: Some(root.clone()),
            ..ServerState::default()
        };
        let files = cargo_diagnostics_by_uri(
            &[diagnostic, failure_note],
            false,
            &state,
            "file:///fallback.ox",
            "fallback",
        );
        assert_eq!(files.len(), 1);
        assert_eq!(files[0]["diagnostics"].as_array().unwrap().len(), 1);
        assert_eq!(
            files[0]["diagnostics"][0]["range"]["start"],
            json!({"line": 2, "character": expected_character})
        );
        std::fs::remove_dir_all(root).expect("temporary fixture directory is removed");
    }

    #[test]
    fn successful_cargo_check_without_diagnostics_has_no_synthetic_error() {
        let state = ServerState::default();
        let files = cargo_diagnostics_by_uri(&[], true, &state, "file:///main.ox", "");
        assert!(files.is_empty());
    }

    #[test]
    fn exposes_resource_stats_and_formats_indented_documents() {
        let mut state = configured_test_state();
        handle(
            json!({"jsonrpc":"2.0","method":"textDocument/didOpen","params":{"textDocument":{"uri":"file:///main.ox","text":"sintaxis indentacion\nfuncion principal():\n  pass\n"}}}),
            &mut state,
        );
        let formatting = handle(
            json!({"jsonrpc":"2.0","id":11,"method":"textDocument/formatting","params":{"textDocument":{"uri":"file:///main.ox"}}}),
            &mut state,
        );
        assert_eq!(
            formatting[0]["result"][0]["newText"],
            "sintaxis indentacion\nfuncion principal():\n    pass\n"
        );
    }

    #[test]
    fn lsp_formatting_uses_the_same_indented_formatter_as_the_cli() {
        let mut state = configured_test_state();
        let original = "sintaxis indentacion\nfuncion suma(a:i32,b:i32)->i32:   \n  retornar a+b\n";
        let expected =
            "sintaxis indentacion\nfuncion suma(a: i32, b: i32) -> i32:\n    retornar a + b\n";
        handle(
            json!({"jsonrpc":"2.0","method":"textDocument/didOpen","params":{"textDocument":{"uri":"file:///suma.ox","text":original}}}),
            &mut state,
        );
        let formatting = handle(
            json!({"jsonrpc":"2.0","id":71,"method":"textDocument/formatting","params":{"textDocument":{"uri":"file:///suma.ox"}}}),
            &mut state,
        );
        assert_eq!(formatting[0]["result"][0]["newText"], expected);
        handle(
            json!({"jsonrpc":"2.0","method":"textDocument/didChange","params":{"textDocument":{"uri":"file:///suma.ox","version":2},"contentChanges":[{"text":expected}]}}),
            &mut state,
        );
        let formatting = handle(
            json!({"jsonrpc":"2.0","id":72,"method":"textDocument/formatting","params":{"textDocument":{"uri":"file:///suma.ox"}}}),
            &mut state,
        );
        assert_eq!(formatting[0]["result"], json!([]));
    }

    #[test]
    fn uses_real_rust_analyzer_when_workspace_is_trusted() {
        if std::process::Command::new(
            std::env::var_os("OXIDO_RUST_ANALYZER").unwrap_or_else(|| "rust-analyzer".into()),
        )
        .arg("--version")
        .output()
        .is_err()
        {
            return;
        }
        let before = RA_RESPONSES.load(Ordering::Relaxed);
        let mut state = ServerState {
            trusted_workspace: true,
            project_mode: Some(oxido_language::domain::syntax::SyntaxMode::Braces),
            ..ServerState::default()
        };
        let text = "fn principal() {\n    let valor = 1;\n}\n";
        state.documents.insert(
            "file:///main.ox".into(),
            TextDocument {
                version: Some(1),
                text: text.into(),
            },
        );
        let response = hover_response(
            json!(1),
            Some(
                &json!({"textDocument":{"uri":"file:///main.ox"},"position":{"line":1,"character":8}}),
            ),
            &state,
        );
        assert_eq!(response["id"], 1);
        assert!(response.get("result").is_some() || response.get("error").is_some());
        assert!(RA_RESPONSES.load(Ordering::Relaxed) > before);
    }

    #[test]
    fn reuses_one_rust_analyzer_session_for_multiple_queries() {
        if std::process::Command::new(
            std::env::var_os("OXIDO_RUST_ANALYZER").unwrap_or_else(|| "rust-analyzer".into()),
        )
        .arg("--version")
        .output()
        .is_err()
        {
            return;
        }
        let before = RA_STARTS.load(Ordering::Relaxed);
        let mut state = ServerState {
            trusted_workspace: true,
            project_mode: Some(oxido_language::domain::syntax::SyntaxMode::Braces),
            ..ServerState::default()
        };
        let text = "fn principal() { let valor = 1; }\n";
        state.documents.insert(
            "file:///main.ox".into(),
            TextDocument {
                version: Some(1),
                text: text.into(),
            },
        );
        let params =
            json!({"textDocument":{"uri":"file:///main.ox"},"position":{"line":0,"character":25}});
        let hover = hover_response(json!(31), Some(&params), &state);
        assert_eq!(hover["id"], 31);
        let completion = completion_response(json!(32), Some(&params), &state);
        assert_eq!(completion["id"], 32);
        let starts = RA_STARTS.load(Ordering::Relaxed);
        assert!(starts > before);
    }

    #[test]
    fn untrusted_workspace_never_starts_rust_analyzer() {
        let before = RA_STARTS.load(Ordering::Relaxed);
        let mut state = configured_test_state();
        state.documents.insert(
            "file:///main.ox".to_owned(),
            TextDocument {
                version: Some(1),
                text: "funcion principal():\n    dejar valor = 1\n".to_owned(),
            },
        );
        let params =
            json!({"textDocument":{"uri":"file:///main.ox"},"position":{"line":1,"character":10}});
        let hover = hover_response(json!(33), Some(&params), &state);
        assert_eq!(hover["id"], 33);
        let completion = completion_response(json!(34), Some(&params), &state);
        assert_eq!(completion["id"], 34);
        assert_eq!(RA_STARTS.load(Ordering::Relaxed), before);
    }

    #[test]
    fn rust_analyzer_configuration_response_preserves_request_cardinality() {
        let response = rust_analyzer_client_response(
            &json!(12),
            &json!({
                "method":"workspace/configuration",
                "params":{"items":[{"section":"rust-analyzer"},{"section":"cargo"}]}
            }),
        );
        assert_eq!(response["id"], 12);
        assert_eq!(response["result"], json!([null, null]));
    }

    #[test]
    fn maps_rust_analyzer_locations_back_to_ox_uris_and_utf16_ranges() {
        let generated_root = PathBuf::from("/tmp/oxido-ra-test");
        let source_text = "funcion principal() {}\n";
        let generated = oxido_compiler::generate_ox(
            &SourceFile::new(source_text),
            LexOptions {
                project_mode: Some(oxido_language::domain::syntax::SyntaxMode::Braces),
            },
        )
        .expect("source map fixture generates Rust");
        let mut files = BTreeMap::new();
        files.insert(PathBuf::from("main.rs"), generated.rust);
        let mut source_maps = BTreeMap::new();
        source_maps.insert(
            PathBuf::from("main.rs"),
            RustAnalyzerSourceMap {
                uri: "file:///proyecto/main.ox".to_owned(),
                text: source_text.to_owned(),
                map: generated.source_map,
            },
        );
        let workspace = RustAnalyzerWorkspace {
            files,
            source_maps,
            requested_path: PathBuf::from("main.rs"),
        };
        let mut response = json!({
            "uri":"file:///tmp/oxido-ra-test/src/main.rs",
            "range":{"start":{"line":0,"character":0},"end":{"line":0,"character":1}}
        });
        map_rust_analyzer_locations(&mut response, &workspace, &generated_root);
        assert_eq!(response["uri"], "file:///proyecto/main.ox");
        assert_eq!(response["range"]["start"], json!({"line":0,"character":0}));
    }

    #[test]
    fn rust_analyzer_wait_uses_one_deadline_despite_notifications() {
        let (sender, receiver) = mpsc::channel();
        let producer = thread::spawn(move || {
            for _ in 0..8 {
                if sender
                    .send(json!({"jsonrpc":"2.0","method":"$/progress","params":{}}))
                    .is_err()
                {
                    break;
                }
                thread::sleep(Duration::from_millis(10));
            }
        });
        let started = Instant::now();
        assert!(wait_response_with_timeout(&receiver, 7, Duration::from_millis(35)).is_none());
        assert!(started.elapsed() < Duration::from_millis(70));
        assert!(producer.join().is_ok());
    }

    #[test]
    fn resolves_definitions_and_references_across_open_ox_modules() {
        let mut state = ServerState::default();
        let module_text = "sintaxis indentacion\nfuncion cargar():\n    pass\n";
        let root_text = "sintaxis indentacion\nfuncion principal():\n    cargar()\n";
        state.documents.insert(
            "file:///src/modulo.ox".to_owned(),
            TextDocument {
                version: Some(1),
                text: module_text.to_owned(),
            },
        );
        state.documents.insert(
            "file:///src/main.ox".to_owned(),
            TextDocument {
                version: Some(1),
                text: root_text.to_owned(),
            },
        );
        let definition = definition_response(
            json!(41),
            Some(
                &json!({"textDocument":{"uri":"file:///src/main.ox"},"position":{"line":2,"character":5}}),
            ),
            &state,
        );
        assert_eq!(definition["result"][0]["uri"], "file:///src/modulo.ox");
        let references = references_response(
            json!(42),
            Some(
                &json!({"textDocument":{"uri":"file:///src/main.ox"},"position":{"line":2,"character":5}}),
            ),
            &state,
        );
        assert!(references["result"].as_array().is_some_and(|items| {
            items
                .iter()
                .any(|item| item["uri"] == "file:///src/modulo.ox")
        }));
        let completion = completion_response(
            json!(43),
            Some(
                &json!({"textDocument":{"uri":"file:///src/main.ox"},"position":{"line":2,"character":5}}),
            ),
            &state,
        );
        assert!(
            completion["result"]["items"]
                .as_array()
                .is_some_and(|items| items.iter().any(|item| item["label"] == "cargar"))
        );
    }

    fn navigation_fixture_state() -> (ServerState, String, String) {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../editors/vscode-oxido/test/fixtures/navigation")
            .canonicalize()
            .expect("navigation fixture root exists");
        let main_path = root.join("main.ox");
        let main_uri = file_uri(&main_path);
        let main_text =
            std::fs::read_to_string(&main_path).expect("navigation fixture is readable");
        let mut state = ServerState {
            project_root: Some(root),
            project_mode: Some(oxido_language::domain::syntax::SyntaxMode::Indentation),
            ..ServerState::default()
        };
        state.documents.insert(
            main_uri.clone(),
            TextDocument {
                version: Some(1),
                text: main_text.clone(),
            },
        );
        (state, main_uri, main_text)
    }

    fn params_for_nth(uri: &str, text: &str, needle: &str, occurrence: usize) -> Value {
        let mut search = 0_usize;
        let mut found = None;
        for _ in 0..=occurrence {
            let relative = text
                .get(search..)
                .and_then(|rest| rest.find(needle))
                .expect("needle exists");
            found = Some(search + relative);
            search += relative + needle.len();
        }
        let position = lsp_position(text, found.expect("needle exists")).expect("offset maps");
        json!({"textDocument":{"uri":uri},"position":position})
    }

    #[test]
    fn resolves_qualified_repeated_functions_to_distinct_modules() {
        let (state, uri, text) = navigation_fixture_state();
        let alpha = definition_response(
            json!(51),
            Some(&params_for_nth(&uri, &text, "repetida", 0)),
            &state,
        );
        assert!(
            alpha["result"][0]["uri"]
                .as_str()
                .is_some_and(|uri| uri.ends_with("/alpha.ox")),
            "alpha definition: {alpha}"
        );
        let beta = definition_response(
            json!(52),
            Some(&params_for_nth(&uri, &text, "repetida", 1)),
            &state,
        );
        assert!(
            beta["result"][0]["uri"]
                .as_str()
                .is_some_and(|uri| uri.ends_with("/beta.ox")),
            "beta definition: {beta}"
        );
    }

    #[test]
    fn resolves_nested_type_and_associated_method_through_qualified_path() {
        let (state, uri, text) = navigation_fixture_state();
        let structure = definition_response(
            json!(53),
            Some(&params_for_nth(&uri, &text, "Alumno", 0)),
            &state,
        );
        assert!(
            structure["result"][0]["uri"]
                .as_str()
                .is_some_and(|uri| uri.ends_with("/dominio/modelos.ox")),
            "type definition: {structure}"
        );
        let method = definition_response(
            json!(54),
            Some(&params_for_nth(&uri, &text, "nuevo", 0)),
            &state,
        );
        assert!(
            method["result"][0]["uri"]
                .as_str()
                .is_some_and(|uri| uri.ends_with("/dominio/modelos.ox")),
            "method definition: {method}"
        );
        let hover = hover_response(
            json!(55),
            Some(&params_for_nth(&uri, &text, "nuevo", 0)),
            &state,
        );
        assert!(
            hover["result"]["contents"]["value"]
                .as_str()
                .is_some_and(|value| value.contains("método")),
            "method hover: {hover}"
        );
    }

    #[test]
    fn repeated_references_do_not_cross_into_another_qualified_module() {
        let (state, uri, text) = navigation_fixture_state();
        let references = references_response(
            json!(56),
            Some(&params_for_nth(&uri, &text, "repetida", 0)),
            &state,
        );
        let uris = references["result"]
            .as_array()
            .expect("references array")
            .iter()
            .filter_map(|location| location["uri"].as_str())
            .collect::<Vec<_>>();
        assert!(uris.iter().any(|uri| uri.ends_with("/alpha.ox")));
        assert!(!uris.iter().any(|uri| uri.ends_with("/beta.ox")));
    }

    #[test]
    fn resolves_module_declaration_and_reverse_references_across_ox_files() {
        let (mut state, uri, text) = navigation_fixture_state();
        let module = definition_response(
            json!(61),
            Some(&params_for_nth(&uri, &text, "modelos", 0)),
            &state,
        );
        assert!(
            module["result"][0]["uri"]
                .as_str()
                .is_some_and(|uri| uri.ends_with("/dominio.ox")),
            "module declaration: {module}"
        );

        let root = PathBuf::from(uri.strip_prefix("file://").expect("file uri"))
            .parent()
            .expect("fixture directory")
            .to_path_buf();
        let alpha_path = root.join("alpha.ox");
        let alpha_uri = file_uri(&alpha_path);
        let alpha_text = std::fs::read_to_string(&alpha_path).expect("alpha fixture is readable");
        state.documents.insert(
            alpha_uri.clone(),
            TextDocument {
                version: Some(1),
                text: alpha_text.clone(),
            },
        );

        let references = references_response(
            json!(62),
            Some(&params_for_nth(&alpha_uri, &alpha_text, "repetida", 0)),
            &state,
        );
        let uris = references["result"]
            .as_array()
            .expect("references array")
            .iter()
            .filter_map(|location| location["uri"].as_str())
            .collect::<Vec<_>>();
        assert!(
            uris.iter().any(|uri| *uri == alpha_uri),
            "declaration: {references}"
        );
        assert!(
            uris.iter().any(|uri| uri.ends_with("/main.ox")),
            "qualified call from main: {references}"
        );
        assert!(
            !uris.iter().any(|uri| uri.ends_with("/beta.ox")),
            "alpha references do not cross into beta: {references}"
        );
    }

    #[test]
    fn ambiguous_unqualified_name_reports_every_cross_module_candidate() {
        let (mut state, uri, _text) = navigation_fixture_state();
        let root = PathBuf::from(uri.strip_prefix("file://").expect("file uri"))
            .parent()
            .expect("fixture directory")
            .to_path_buf();
        let ambigua_path = root.join("ambigua.ox");
        let ambigua_uri = file_uri(&ambigua_path);
        let ambigua_text =
            std::fs::read_to_string(&ambigua_path).expect("ambiguous fixture is readable");
        state.documents.insert(
            ambigua_uri.clone(),
            TextDocument {
                version: Some(1),
                text: ambigua_text.clone(),
            },
        );
        let params = params_for_nth(&ambigua_uri, &ambigua_text, "repetida", 0);

        let definition = definition_response(json!(63), Some(&params), &state);
        let locations = definition["result"].as_array().expect("definition array");
        assert_eq!(
            locations.len(),
            2,
            "both modules are candidates: {definition}"
        );
        assert!(locations.iter().any(|location| {
            location["uri"]
                .as_str()
                .is_some_and(|uri| uri.ends_with("/alpha.ox"))
        }));
        assert!(locations.iter().any(|location| {
            location["uri"]
                .as_str()
                .is_some_and(|uri| uri.ends_with("/beta.ox"))
        }));

        let hover = hover_response(json!(64), Some(&params), &state);
        let value = hover["result"]["contents"]["value"]
            .as_str()
            .expect("ambiguous hover text");
        assert!(
            value.contains("2 declaraciones candidatas"),
            "hover: {hover}"
        );
        assert!(
            value.contains("alpha.ox") && value.contains("beta.ox"),
            "hover: {hover}"
        );

        let references = references_response(json!(65), Some(&params), &state);
        let uris = references["result"]
            .as_array()
            .expect("references array")
            .iter()
            .filter_map(|location| location["uri"].as_str())
            .collect::<Vec<_>>();
        assert!(
            uris.iter().any(|uri| uri.ends_with("/alpha.ox"))
                && uris.iter().any(|uri| uri.ends_with("/beta.ox")),
            "references keep both candidates: {references}"
        );
    }

    #[test]
    fn completion_and_document_symbols_include_types_methods_and_modules() {
        let (mut state, uri, _text) = navigation_fixture_state();
        let models_path = std::path::PathBuf::from(uri.strip_prefix("file://").expect("file uri"))
            .parent()
            .expect("fixture directory")
            .join("dominio/modelos.ox");
        state.documents.insert(
            file_uri(&models_path),
            TextDocument {
                version: Some(1),
                text: std::fs::read_to_string(&models_path).expect("models fixture is readable"),
            },
        );
        let completion = completion_response(
            json!(57),
            Some(&json!({"textDocument":{"uri":uri},"position":{"line":0,"character":0}})),
            &state,
        );
        let labels = completion["result"]["items"]
            .as_array()
            .expect("completion items")
            .iter()
            .filter_map(|item| item["label"].as_str())
            .collect::<BTreeSet<_>>();
        assert!(labels.contains("Alumno"));
        assert!(labels.contains("nuevo"));
        assert!(labels.contains("repetida"));

        let document_symbols = document_symbols_response(
            json!(58),
            Some(&json!({"textDocument":{"uri":file_uri(&models_path)}})),
            &state,
        );
        let symbols = document_symbols["result"]
            .as_array()
            .expect("document symbols")
            .iter()
            .collect::<Vec<_>>();
        assert!(
            symbols
                .iter()
                .any(|symbol| { symbol["name"] == "Alumno" && symbol["kind"] == 23 })
        );
        assert!(
            symbols
                .iter()
                .any(|symbol| { symbol["name"] == "nuevo" && symbol["kind"] == 6 })
        );
    }

    #[test]
    fn indexes_closed_ox_modules_from_the_workspace_root() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../prueba/modulos_jerarquicos/fuente")
            .canonicalize()
            .expect("module fixture root exists");
        let state = ServerState {
            project_root: Some(root),
            ..ServerState::default()
        };
        let symbols = workspace_symbols(&state);
        assert!(
            symbols
                .iter()
                .any(|symbol| symbol.uri.ends_with("/dominio/modelos.ox")
                    && symbol.name == "nuevo"
                    && symbol.container.as_deref() == Some("Alumno"))
        );
        assert!(
            symbols
                .iter()
                .any(|symbol| symbol.uri.ends_with("/dominio/modelos.ox")
                    && symbol.name == "Alumno"
                    && symbol.kind == SymbolKind::Struct)
        );
    }

    #[test]
    fn falls_back_to_project_module_index_when_rust_analyzer_has_no_definition() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../editors/vscode-oxido/test/fixtures/trusted")
            .canonicalize()
            .expect("trusted editor fixture exists");
        let main_path = root.join("semantic_modulo.ox");
        let main_uri = file_uri(&main_path);
        let main_text = std::fs::read_to_string(&main_path).expect("main fixture is readable");
        let mut state = ServerState {
            project_root: Some(root),
            ..ServerState::default()
        };
        state.documents.insert(
            main_uri.clone(),
            TextDocument {
                version: Some(1),
                text: main_text,
            },
        );

        let response = definition_response(
            json!(44),
            Some(&json!({
                "textDocument":{"uri":main_uri},
                "position":{"line":4,"character":12}
            })),
            &state,
        );

        assert!(
            response["result"].as_array().is_some_and(|locations| {
                locations.iter().any(|location| {
                    location["uri"]
                        .as_str()
                        .is_some_and(|uri| uri.ends_with("/util.ox"))
                })
            }),
            "response: {response}"
        );
    }

    #[test]
    fn rust_analyzer_workspace_preserves_the_resolved_ox_module_tree() {
        let project_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../prueba/modulos_jerarquicos")
            .canonicalize()
            .expect("module fixture root exists");
        let root_path = project_root.join("fuente/main.ox");
        let source = SourceFile::with_path(
            &root_path,
            std::fs::read_to_string(&root_path).expect("module fixture root is readable"),
        );
        let state = ServerState {
            project_root: Some(project_root),
            project_mode: Some(oxido_language::domain::syntax::SyntaxMode::Braces),
            ..ServerState::default()
        };
        let generated = oxido_compiler::generate_ox(
            &source,
            LexOptions {
                project_mode: state.project_mode,
            },
        )
        .expect("module fixture root generates Rust");
        let workspace = rust_analyzer_workspace(&state, &file_uri(&root_path), &source, generated)
            .expect("module fixture resolves into a Rust workspace");
        assert_eq!(workspace.requested_path, PathBuf::from("main.rs"));
        assert!(
            workspace
                .files
                .contains_key(&PathBuf::from("aplicacion.rs"))
        );
        assert!(
            workspace
                .files
                .contains_key(&PathBuf::from("aplicacion/servicios.rs"))
        );
        assert!(workspace.files.contains_key(&PathBuf::from("dominio.rs")));
        assert!(
            workspace
                .files
                .contains_key(&PathBuf::from("dominio/modelos.rs"))
        );
    }

    #[test]
    fn cancel_request_notification_marks_the_matching_request_token() {
        let cancellation = Arc::new(AtomicBool::new(false));
        let pending = HashMap::from([(
            "42".to_owned(),
            PendingSemanticRequest {
                id: json!(42),
                uri: None,
                document_version: None,
                document_text: String::new(),
                cancellation: Arc::clone(&cancellation),
            },
        )]);
        let notification = json!({
            "jsonrpc":"2.0",
            "method":"$/cancelRequest",
            "params":{"id":42}
        });

        assert!(cancel_request(
            &notification,
            &pending,
            &HashMap::<String, PendingCargoRequest>::new()
        ));
        assert!(cancellation.load(Ordering::Relaxed));
        assert_eq!(
            canceled_response(json!(42))["error"]["code"],
            CANCELED_ERROR_CODE
        );
    }

    #[test]
    fn cancel_request_notification_marks_a_pending_cargo_check() {
        let cancellation = Arc::new(AtomicBool::new(false));
        let pending_cargo = HashMap::from([(
            "43".to_owned(),
            PendingCargoRequest {
                id: json!(43),
                cancellation: Arc::clone(&cancellation),
                snapshot: CargoCheckSnapshot {
                    uri: "file:///main.ox".to_owned(),
                    document_version: Some(1),
                    document_text: String::new(),
                    source_path: None,
                    project_root: None,
                    project_mode: None,
                },
            },
        )]);
        let notification = json!({
            "jsonrpc":"2.0",
            "method":"$/cancelRequest",
            "params":{"id":43}
        });
        assert!(cancel_request(
            &notification,
            &HashMap::<String, PendingSemanticRequest>::new(),
            &pending_cargo
        ));
        assert!(cancellation.load(Ordering::Relaxed));
    }

    #[test]
    fn canceled_semantic_token_work_stops_and_returns_request_cancelled() {
        let cancellation = AtomicBool::new(true);
        assert_eq!(
            semantic_tokens_cancellable(
                "funcion a():\nfuncion b():\n",
                Some(oxido_language::domain::syntax::SyntaxMode::Indentation),
                &cancellation
            ),
            None
        );
        assert_eq!(
            canceled_response(json!(7))["error"]["code"],
            CANCELED_ERROR_CODE
        );
    }

    #[test]
    fn accepts_the_frame_size_boundary_and_rejects_oversized_messages() {
        assert!(validate_frame_length(MAX_MESSAGE_BYTES).is_ok());
        assert!(validate_frame_length(MAX_MESSAGE_BYTES + 1).is_err());
    }

    #[test]
    fn reads_a_complete_frame_and_handles_end_of_stream() {
        let body = br#"{"jsonrpc":"2.0","id":1,"method":"shutdown"}"#;
        let mut wire = format!("Content-Length: {}\r\n\r\n", body.len()).into_bytes();
        wire.extend_from_slice(body);
        let mut input = io::Cursor::new(wire);
        assert_eq!(
            read_frame(&mut input).unwrap().as_deref(),
            Some(body.as_slice())
        );
        assert!(read_frame(&mut input).unwrap().is_none());
    }

    #[test]
    fn stdio_server_processes_requests_and_writes_framed_responses() {
        let requests = [
            br#"{"jsonrpc":"2.0","id":1,"method":"initialize"}"#.as_slice(),
            br#"{"jsonrpc":"2.0","id":2,"method":"shutdown"}"#.as_slice(),
        ];
        let mut input = Vec::new();
        for request in requests {
            input
                .extend_from_slice(format!("Content-Length: {}\r\n\r\n", request.len()).as_bytes());
            input.extend_from_slice(request);
        }
        let mut output = Vec::new();
        serve_stdio(io::Cursor::new(input), &mut output).expect("stdio LSP loop succeeds");

        let mut output = io::Cursor::new(output);
        let initialize = read_frame(&mut output)
            .expect("initialize response frame is valid")
            .expect("initialize response exists");
        let shutdown = read_frame(&mut output)
            .expect("shutdown response frame is valid")
            .expect("shutdown response exists");
        assert_eq!(
            serde_json::from_slice::<Value>(&initialize).expect("initialize JSON is valid")["id"],
            1
        );
        assert_eq!(
            serde_json::from_slice::<Value>(&shutdown).expect("shutdown JSON is valid")["id"],
            2
        );
        assert!(
            read_frame(&mut output)
                .expect("output EOF is valid")
                .is_none()
        );
    }

    #[test]
    fn rejects_duplicate_missing_oversized_and_truncated_frames() {
        let mut duplicate = io::Cursor::new(b"Content-Length: 0\r\nContent-Length: 0\r\n\r\n");
        assert!(read_frame(&mut duplicate).is_err());

        let mut missing = io::Cursor::new(b"Other: value\r\n\r\n");
        assert!(read_frame(&mut missing).is_err());

        let oversized_header = format!("Content-Length: {}\r\n\r\n", MAX_MESSAGE_BYTES + 1);
        let mut oversized = io::Cursor::new(oversized_header.into_bytes());
        assert!(read_frame(&mut oversized).is_err());

        let mut truncated = io::Cursor::new(b"Content-Length: 5\r\n\r\n{}" as &[u8]);
        assert_eq!(
            read_frame(&mut truncated).unwrap_err().kind(),
            io::ErrorKind::UnexpectedEof
        );
    }

    #[test]
    fn rejects_a_header_line_over_the_limit_without_reading_the_body() {
        let bytes = vec![b'x'; MAX_HEADER_LINE_BYTES + 1];
        let mut input = io::Cursor::new(bytes);
        assert_eq!(
            read_frame(&mut input).unwrap_err().kind(),
            io::ErrorKind::InvalidData
        );
    }
}
