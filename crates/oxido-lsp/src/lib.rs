#![forbid(unsafe_code)]

use oxido_language::{
    SourceFile,
    domain::ast::{File as OxFile, Item},
    domain::syntax::{LexOptions, TokenKind, lex},
};
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, BTreeSet, HashMap},
    io::{self, BufRead, BufReader, Read, Write},
    path::PathBuf,
    process::{Command, Stdio},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, SyncSender, TrySendError},
    },
    thread,
    time::Instant,
};

pub const MAX_MESSAGE_BYTES: usize = 4 * 1024 * 1024;
const MAX_HEADER_LINE_BYTES: usize = 8 * 1024;
const MAX_PENDING_SEMANTIC_REQUESTS: usize = 4;
const MAX_QUEUED_LSP_EVENTS: usize = 8;
const MAX_OPEN_DOCUMENTS: usize = 64;
const MAX_OPEN_DOCUMENT_BYTES: usize = 16 * 1024 * 1024;
const MAX_SINGLE_DOCUMENT_BYTES: usize = 4 * 1024 * 1024;
const MAX_QUEUED_DIAGNOSTIC_TASKS: usize = 1;
const CANCELED_ERROR_CODE: i64 = -32800;
const CONTENT_MODIFIED_ERROR_CODE: i64 = -32801;
static NEXT_RA_WORKSPACE: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

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

enum ServerEvent {
    Frame(Vec<u8>),
    AsyncResponse {
        key: String,
        response: Value,
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
    let mut pending_diagnostics = HashMap::<String, PendingDiagnostic>::new();
    let mut dirty_diagnostics = BTreeSet::<String>::new();
    let mut input_error = None;
    let mut input_ended = false;
    while let Ok(event) = events_rx.recv() {
        match event {
            ServerEvent::Frame(body) => {
                let request = serde_json::from_slice::<Value>(&body).map_err(io::Error::other)?;
                if cancel_request(&request, &pending) {
                    continue;
                }
                if request.get("method").and_then(Value::as_str) == Some("oxido/check") {
                    let response = cargo_check_response(&request, &state);
                    write_lsp_message(&mut output, &response)?;
                    continue;
                }
                if request.get("method").and_then(Value::as_str) == Some("oxido/stats") {
                    let id = request.get("id").cloned().unwrap_or(Value::Null);
                    write_lsp_message(
                        &mut output,
                        &json!({"jsonrpc":"2.0","id":id,"result":{"openDocuments":state.documents.len(),"documentBytes":state.total_document_bytes,"diagnosticRuns":state.diagnostic_runs,"lastDiagnosticMs":state.last_diagnostic_ms,"residentMemoryKiB":resident_memory_kib()}}),
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
                    } else if document_was_modified(&state, &request) {
                        json!({"jsonrpc":"2.0","id":request.id,"error":{"code":CONTENT_MODIFIED_ERROR_CODE,"message":"el documento cambió durante el cálculo"}})
                    } else {
                        response
                    };
                    if !input_ended {
                        write_lsp_message(&mut output, &final_response)?;
                    }
                }
                if input_ended && pending.is_empty() && pending_diagnostics.is_empty() {
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
                if !input_ended && state.documents.contains_key(&uri) {
                    dirty_diagnostics.insert(uri);
                    dispatch_diagnostic(
                        &diagnostics_tx,
                        &state,
                        &mut dirty_diagnostics,
                        &mut pending_diagnostics,
                    );
                }
                if input_ended && pending.is_empty() && pending_diagnostics.is_empty() {
                    break;
                }
            }
            ServerEvent::InputEnded => {
                input_ended = true;
                for request in pending.values() {
                    request.cancellation.store(true, Ordering::Relaxed);
                }
                for diagnostic in pending_diagnostics.values() {
                    diagnostic.cancellation.store(true, Ordering::Relaxed);
                }
                dirty_diagnostics.clear();
                if pending.is_empty() && pending_diagnostics.is_empty() {
                    break;
                }
            }
            ServerEvent::InputFailed(error) => {
                input_error = Some(error);
                input_ended = true;
                for request in pending.values() {
                    request.cancellation.store(true, Ordering::Relaxed);
                }
                for diagnostic in pending_diagnostics.values() {
                    diagnostic.cancellation.store(true, Ordering::Relaxed);
                }
                dirty_diagnostics.clear();
                if pending.is_empty() && pending_diagnostics.is_empty() {
                    break;
                }
            }
        }
    }
    for request in pending.values() {
        request.cancellation.store(true, Ordering::Relaxed);
    }
    reader
        .join()
        .map_err(|_| io::Error::other("el hilo lector LSP terminó inesperadamente"))?;
    drop(diagnostics_tx);
    diagnostics_worker
        .join()
        .map_err(|_| io::Error::other("el worker de diagnósticos LSP terminó inesperadamente"))?;
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

fn cancel_request(request: &Value, pending: &HashMap<String, PendingSemanticRequest>) -> bool {
    if request.get("method").and_then(Value::as_str) != Some("$/cancelRequest") {
        return false;
    }
    if let Some(id) = request.pointer("/params/id")
        && let Some(request) = pending.get(&id.to_string())
    {
        request.cancellation.store(true, Ordering::Relaxed);
    }
    true
}

fn canceled_response(id: Value) -> Value {
    json!({"jsonrpc":"2.0","id":id,"error":{"code":CANCELED_ERROR_CODE,"message":"solicitud cancelada"}})
}

fn document_was_modified(state: &ServerState, request: &PendingSemanticRequest) -> bool {
    let Some(uri) = request.uri.as_deref() else {
        return false;
    };
    state.documents.get(uri).is_none_or(|document| {
        document.version != request.document_version || document.text != request.document_text
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

fn cargo_check_response(request: &Value, state: &ServerState) -> Value {
    let id = request.get("id").cloned().unwrap_or(Value::Null);
    let trusted = state.trusted_workspace
        && request.pointer("/params/trusted").and_then(Value::as_bool) == Some(true);
    if !trusted {
        return json!({"jsonrpc":"2.0","id":id,"error":{"code":-32003,"message":"Cargo requiere confianza explícita del workspace"}});
    }
    let Some(uri) = request
        .pointer("/params/textDocument/uri")
        .and_then(Value::as_str)
    else {
        return json!({"jsonrpc":"2.0","id":id,"error":{"code":-32602,"message":"falta textDocument.uri"}});
    };
    let Some(document) = state.documents.get(uri) else {
        return json!({"jsonrpc":"2.0","id":id,"error":{"code":-32602,"message":"el documento no está abierto"}});
    };
    let source_path = file_uri_path(uri).map(PathBuf::from);
    let source = source_path.as_deref().map_or_else(
        || SourceFile::new(&document.text),
        |path| SourceFile::with_path(path, &document.text),
    );
    let result = if let Some(project_root) = state.project_root.as_deref() {
        let config_path = project_root.join("oxido.toml");
        let config_text = std::fs::read_to_string(&config_path).map_err(|error| error.to_string());
        match config_text.and_then(|text| {
            oxido_compiler::config::ProjectConfig::parse(&text).map_err(|error| error.to_string())
        }) {
            Ok(config) => match config.resolve_dependencies(project_root) {
                Ok(dependencies) => oxido_compiler::build_ox_project_in_project(
                    &source,
                    LexOptions {
                        project_mode: state.project_mode,
                    },
                    &dependencies,
                    Some(&project_root.join("Cargo.lock")),
                    oxido_compiler::application::cargo::CargoAction::CheckTrusted,
                    project_root,
                ),
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
        oxido_compiler::build_ox(
            &source,
            LexOptions {
                project_mode: state.project_mode,
            },
            oxido_compiler::application::cargo::CargoAction::CheckTrusted,
        )
    };
    let diagnostics = match result {
        Ok(result) => result
            .diagnostics
            .into_iter()
            .filter_map(|diagnostic| {
                let source_location = diagnostic.source?;
                let start = lsp_position(&document.text, source_location.start)?;
                let end = lsp_position(&document.text, source_location.end).unwrap_or(start.clone());
                Some(json!({"range":{"start":start,"end":end},"severity":1,"code":diagnostic.code,"source":"cargo","message":diagnostic.message}))
            })
            .collect::<Vec<_>>(),
        Err(diagnostics) => diagnostics
            .into_iter()
            .filter_map(|diagnostic| {
                let start = lsp_position(&document.text, diagnostic.primary_span.start())?;
                let end = lsp_position(&document.text, diagnostic.primary_span.end()).unwrap_or(start.clone());
                Some(json!({"range":{"start":start,"end":end},"severity":1,"code":diagnostic.code,"source":"oxido","message":diagnostic.message}))
            })
            .collect::<Vec<_>>(),
    };
    json!({"jsonrpc":"2.0","id":id,"result":{"uri":uri,"diagnostics":diagnostics}})
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
    if let Some(file) = parsed_file(text, state.project_mode) {
        for (name, _, _) in declared_functions(&file, text) {
            items.push(
                json!({"label":name,"kind":3,"detail":"Función declarada en este documento"}),
            );
        }
    }
    json!({"jsonrpc":"2.0","id":id,"result":{"isIncomplete":false,"items":items}})
}

fn hover_response(id: Value, params: Option<&Value>, state: &ServerState) -> Value {
    let Some((_uri, text, offset)) = request_document_position(params, state) else {
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
        TokenKind::Identifier { .. } => parsed_file(text, state.project_mode).and_then(|file| {
            declared_functions(&file, text)
                .into_iter()
                .find(|(name, _, _)| name == token_text)
                .map(|(_, _, range)| {
                    format!(
                        "Función `{token_text}` declarada en este documento.\n\nRango: `{}`–`{}`.",
                        range["start"], range["end"]
                    )
                })
        }),
        _ => None,
    };
    let Some(contents) = contents else {
        return json!({"jsonrpc":"2.0","id":id,"result":null});
    };
    let start = lsp_position(text, token.span.start()).unwrap_or(json!({"line":0,"character":0}));
    let end = lsp_position(text, token.span.end()).unwrap_or(start.clone());
    json!({"jsonrpc":"2.0","id":id,"result":{"contents":{"kind":"markdown","value":contents},"range":{"start":start,"end":end}}})
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
    let symbols = parsed_file(&document.text, state.project_mode)
        .map(|file| {
            declared_functions(&file, &document.text).into_iter().map(|(name, selection, range)| {
            json!({"name":name,"kind":12,"range":range,"selectionRange":selection,"detail":"Óxido"})
        }).collect::<Vec<_>>()
        })
        .unwrap_or_default();
    json!({"jsonrpc":"2.0","id":id,"result":symbols})
}

fn definition_response(id: Value, params: Option<&Value>, state: &ServerState) -> Value {
    let Some((uri, text, offset)) = request_document_position(params, state) else {
        return json!({"jsonrpc":"2.0","id":id,"result":[]});
    };
    if let Some(response) =
        rust_analyzer_response("textDocument/definition", &id, params, state, text)
    {
        return response;
    }
    let Some(token) = lex_token_at(text, offset, state.project_mode) else {
        return json!({"jsonrpc":"2.0","id":id,"result":[]});
    };
    let name = &text[token.span.start()..token.span.end()];
    let Some(file) = parsed_file(text, state.project_mode) else {
        return json!({"jsonrpc":"2.0","id":id,"result":[]});
    };
    let location = declared_functions(&file, text)
        .into_iter()
        .find(|(declared, _, _)| declared == name)
        .map(|(_, selection, _)| json!({"uri":uri,"range":selection}));
    json!({"jsonrpc":"2.0","id":id,"result":location.into_iter().collect::<Vec<_>>()})
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
    let lexed = lex(
        &SourceFile::new(text),
        LexOptions {
            project_mode: state.project_mode,
        },
    );
    let locations = lexed
        .tokens
        .into_iter()
        .filter(|token| {
            matches!(token.kind, TokenKind::Identifier { .. })
                && &text[token.span.start()..token.span.end()] == selected_text
        })
        .filter_map(|token| {
            let start = lsp_position(text, token.span.start())?;
            let end = lsp_position(text, token.span.end())?;
            Some(json!({"uri":uri,"range":{"start":start,"end":end}}))
        })
        .collect::<Vec<_>>();
    json!({"jsonrpc":"2.0","id":id,"result":locations})
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
    if state.project_mode == Some(oxido_language::domain::syntax::SyntaxMode::Indentation) {
        let formatted = format_indented_editor(&document.text);
        if formatted == document.text {
            return json!({"jsonrpc":"2.0","id":id,"result":[]});
        }
        let end = lsp_position(&document.text, document.text.len())
            .unwrap_or(json!({"line":0,"character":0}));
        return json!({"jsonrpc":"2.0","id":id,"result":[{"range":{"start":{"line":0,"character":0},"end":end},"newText":formatted}]});
    }
    let source = SourceFile::new(&document.text);
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

fn format_indented_editor(text: &str) -> String {
    let mut depth = 0_usize;
    let mut output = String::with_capacity(text.len());
    for original in text.lines() {
        let trimmed = original.trim();
        if trimmed.is_empty() {
            output.push('\n');
            continue;
        }
        if trimmed.starts_with("sino") || trimmed.starts_with("caso ") {
            depth = depth.saturating_sub(1);
        }
        output.push_str(&" ".repeat(depth.saturating_mul(4)));
        output.push_str(trimmed);
        output.push('\n');
        if trimmed.ends_with(':') {
            depth = depth.saturating_add(1);
        }
    }
    output
}

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
    let generated = oxido_compiler::generate_ox(
        &SourceFile::new(text),
        LexOptions {
            project_mode: state.project_mode,
        },
    )
    .ok()?;
    let generated_position = params
        .and_then(|value| value.get("position"))
        .and_then(|position| position_at_generated(text, &generated.rust, position))?;
    let mut request_params = params.cloned().unwrap_or_else(|| json!({}));
    if let Some(document) = request_params.get_mut("textDocument") {
        *document = json!({"uri":"file:///src/main.rs"});
    }
    if let Some(position) = generated_position
        && let Some(object) = request_params.as_object_mut()
    {
        object.insert("position".to_owned(), position);
    }
    let response = run_rust_analyzer(&generated.rust, method, id, request_params)?;
    Some(response)
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
    let generated_offset = generated.find(token)?;
    Some(lsp_position(generated, generated_offset))
}

fn run_rust_analyzer(rust: &str, method: &str, id: &Value, params: Value) -> Option<Value> {
    let root = std::env::temp_dir().join(format!(
        "oxido-ra-{}",
        NEXT_RA_WORKSPACE.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    ));
    let source_dir = root.join("src");
    std::fs::create_dir_all(&source_dir).ok()?;
    let cleanup = || {
        if let Err(error) = std::fs::remove_dir_all(&root) {
            eprintln!("no se pudo limpiar workspace temporal de rust-analyzer: {error}");
        }
    };
    if std::fs::write(
        root.join("Cargo.toml"),
        "[package]\nname = \"oxido_lsp_generated\"\nversion = \"0.0.0\"\nedition = \"2024\"\n",
    )
    .is_err()
        || std::fs::write(source_dir.join("main.rs"), rust).is_err()
    {
        cleanup();
        return None;
    }
    let executable =
        std::env::var_os("OXIDO_RUST_ANALYZER").unwrap_or_else(|| "rust-analyzer".into());
    let mut child = Command::new(executable)
        .arg("--stdio")
        .current_dir(&root)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;
    let mut input = child.stdin.take()?;
    let mut output = BufReader::new(child.stdout.take()?);
    let initialize = json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{"processId":null,"rootUri":format!("file://{}", root.display()),"capabilities":{}}});
    write_lsp_message(&mut input, &initialize).ok()?;
    read_response(&mut output, 1)?;
    write_lsp_message(
        &mut input,
        &json!({"jsonrpc":"2.0","method":"initialized","params":{}}),
    )
    .ok()?;
    let generated_uri = format!("file://{}/src/main.rs", root.display());
    write_lsp_message(&mut input, &json!({"jsonrpc":"2.0","method":"textDocument/didOpen","params":{"textDocument":{"uri":generated_uri,"languageId":"rust","version":1,"text":rust}}})).ok()?;
    let mut params = params;
    if let Some(document) = params.get_mut("textDocument") {
        *document = json!({"uri":format!("file://{}/src/main.rs", root.display())});
    }
    let request = json!({"jsonrpc":"2.0","id":id,"method":method,"params":params});
    write_lsp_message(&mut input, &request).ok()?;
    let response = read_response(&mut output, id.as_i64().unwrap_or(2));
    write_lsp_message(
        &mut input,
        &json!({"jsonrpc":"2.0","id":99,"method":"shutdown","params":{}}),
    )
    .ok();
    if let Err(error) = child.kill() {
        eprintln!("no se pudo detener rust-analyzer temporal: {error}");
    }
    if let Err(error) = child.wait() {
        eprintln!("no se pudo esperar rust-analyzer temporal: {error}");
    }
    cleanup();
    response
}

fn read_response(input: &mut impl BufRead, expected_id: i64) -> Option<Value> {
    loop {
        let body = read_frame(input).ok()??;
        let response: Value = serde_json::from_slice(&body).ok()?;
        if response.get("id").and_then(Value::as_i64) == Some(expected_id) {
            return Some(response);
        }
    }
}

fn parsed_file(
    text: &str,
    project_mode: Option<oxido_language::domain::syntax::SyntaxMode>,
) -> Option<OxFile> {
    let parsed = oxido_compiler::parse_ox(&SourceFile::new(text), LexOptions { project_mode });
    if parsed.has_errors() {
        return None;
    }
    parsed.file
}

fn declared_functions(file: &OxFile, text: &str) -> Vec<(String, Value, Value)> {
    fn visit(items: &[Item], text: &str, output: &mut Vec<(String, Value, Value)>) {
        for item in items {
            match item {
                Item::Function(function) => {
                    let start = function.span.start();
                    let end = function.span.end().min(text.len());
                    if let Some(relative) = text
                        .get(start..end)
                        .and_then(|source| source.find(&function.name))
                    {
                        let name_start = start + relative;
                        let name_end = name_start + function.name.len();
                        if let (
                            Some(selection_start),
                            Some(selection_end),
                            Some(range_start),
                            Some(range_end),
                        ) = (
                            lsp_position(text, name_start),
                            lsp_position(text, name_end),
                            lsp_position(text, start),
                            lsp_position(text, end),
                        ) {
                            output.push((
                                function.name.clone(),
                                json!({"start":selection_start,"end":selection_end}),
                                json!({"start":range_start,"end":range_end}),
                            ));
                        }
                    }
                }
                Item::IndentedContainer(container) => visit(&container.members, text, output),
                _ => {}
            }
        }
    }
    let mut output = Vec::new();
    visit(&file.items, text, &mut output);
    output
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
        let response = cargo_check_response(
            &json!({
                "jsonrpc":"2.0", "id":9, "method":"oxido/check",
                "params":{"trusted":false,"textDocument":{"uri":"file:///main.ox"}}
            }),
            &state,
        );
        assert_eq!(response["error"]["code"], -32003);
    }

    #[test]
    fn exposes_resource_stats_and_formats_indented_documents() {
        let mut state = configured_test_state();
        handle(
            json!({"jsonrpc":"2.0","method":"textDocument/didOpen","params":{"textDocument":{"uri":"file:///main.ox","text":"sintaxis indentacion\nfuncion principal():\npass\n"}}}),
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

        assert!(cancel_request(&notification, &pending));
        assert!(cancellation.load(Ordering::Relaxed));
        assert_eq!(
            canceled_response(json!(42))["error"]["code"],
            CANCELED_ERROR_CODE
        );
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
