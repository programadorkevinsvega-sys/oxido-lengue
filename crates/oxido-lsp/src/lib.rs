#![forbid(unsafe_code)]

use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, HashMap},
    io::{self, BufRead, BufReader, Read, Write},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    thread,
};

pub const MAX_MESSAGE_BYTES: usize = 4 * 1024 * 1024;
const MAX_HEADER_LINE_BYTES: usize = 8 * 1024;
const MAX_PENDING_SEMANTIC_REQUESTS: usize = 16;
const MAX_QUEUED_LSP_EVENTS: usize = 8;
const CANCELED_ERROR_CODE: i64 = -32800;
const CONTENT_MODIFIED_ERROR_CODE: i64 = -32801;

#[derive(Default)]
pub struct ServerState {
    documents: BTreeMap<String, TextDocument>,
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

enum ServerEvent {
    Frame(Vec<u8>),
    AsyncResponse { key: String, response: Value },
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

    let mut state = ServerState::default();
    let mut pending = HashMap::<String, PendingSemanticRequest>::new();
    let mut input_error = None;
    let mut input_ended = false;
    while let Ok(event) = events_rx.recv() {
        match event {
            ServerEvent::Frame(body) => {
                let request = serde_json::from_slice::<Value>(&body).map_err(io::Error::other)?;
                if cancel_request(&request, &pending) {
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
                    let worker = thread::Builder::new()
                        .name("oxido-lsp-semantic-tokens".to_owned())
                        .spawn(move || {
                            let response = match semantic_tokens_cancellable(
                                &worker_text,
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
                for response in handle(request, &mut state) {
                    write_lsp_message(&mut output, &response)?;
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
                if input_ended && pending.is_empty() {
                    break;
                }
            }
            ServerEvent::InputEnded => {
                input_ended = true;
                for request in pending.values() {
                    request.cancellation.store(true, Ordering::Relaxed);
                }
                if pending.is_empty() {
                    break;
                }
            }
            ServerEvent::InputFailed(error) => {
                input_error = Some(error);
                input_ended = true;
                for request in pending.values() {
                    request.cancellation.store(true, Ordering::Relaxed);
                }
                if pending.is_empty() {
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
    Ok(handle(request, state))
}

fn handle(request: Value, state: &mut ServerState) -> Vec<Value> {
    let Some(method) = request.get("method").and_then(Value::as_str) else {
        return Vec::new();
    };
    let id = request.get("id").cloned();
    let response = match method {
        "initialize" => id.map(|id| json!({"jsonrpc":"2.0","id":id,"result":{"capabilities":{"textDocumentSync":{"openClose":true,"change":2},"semanticTokensProvider":{"full":true,"legend":{"tokenTypes":["keyword"],"tokenModifiers":[]}}}}})),
        "shutdown" => id.map(|id| json!({"jsonrpc":"2.0","id":id,"result":null})),
        "textDocument/semanticTokens/full" => id.map(|id| semantic_response(id, request.get("params"), state)),
        "textDocument/didOpen" => { open_document(&request, state); None }
        "textDocument/didChange" => { change_document(&request, state); None }
        "exit" => None,
        _ => id.map(|id| json!({"jsonrpc":"2.0","id":id,"error":{"code":-32601,"message":"método LSP no soportado"}})),
    };
    let mut messages = response.into_iter().collect::<Vec<_>>();
    if matches!(method, "textDocument/didOpen" | "textDocument/didChange")
        && let Some(uri) = request
            .pointer("/params/textDocument/uri")
            .and_then(Value::as_str)
        && let Some(document) = state.documents.get(uri)
    {
        messages.push(diagnostic_notification(uri, &document.text));
    }
    messages
}

fn diagnostic_notification(uri: &str, text: &str) -> Value {
    let source = oxido_language::SourceFile::new(text);
    let result = oxido_language::domain::syntax::lex(
        &source,
        oxido_language::domain::syntax::LexOptions {
            project_mode: Some(oxido_language::domain::syntax::SyntaxMode::Indentation),
        },
    );
    let diagnostics = result.diagnostics.into_iter().filter_map(|diagnostic| {
        let start = source.position(diagnostic.primary_span.start())?;
        let end = source.position(diagnostic.primary_span.end()).unwrap_or(start);
        Some(json!({"range":{"start":{"line":start.line.saturating_sub(1),"character":start.column.saturating_sub(1)},"end":{"line":end.line.saturating_sub(1),"character":end.column.saturating_sub(1)}},"severity":1,"message":diagnostic.message,"code":diagnostic.code}))
    }).collect::<Vec<_>>();
    json!({"jsonrpc":"2.0","method":"textDocument/publishDiagnostics","params":{"uri":uri,"diagnostics":diagnostics}})
}

fn open_document(request: &Value, state: &mut ServerState) {
    let Some(document) = request.pointer("/params/textDocument") else {
        return;
    };
    let (Some(uri), Some(text)) = (
        document.get("uri").and_then(Value::as_str),
        document.get("text").and_then(Value::as_str),
    ) else {
        return;
    };
    state.documents.insert(
        uri.to_owned(),
        TextDocument {
            version: document.get("version").and_then(Value::as_i64),
            text: text.to_owned(),
        },
    );
}

fn change_document(request: &Value, state: &mut ServerState) {
    let Some(uri) = request
        .pointer("/params/textDocument/uri")
        .and_then(Value::as_str)
    else {
        return;
    };
    let Some(document) = state.documents.get(uri) else {
        return;
    };
    let incoming_version = request
        .pointer("/params/textDocument/version")
        .and_then(Value::as_i64);
    if let (Some(current), Some(incoming)) = (document.version, incoming_version)
        && incoming <= current
    {
        return;
    }
    let Some(changes) = request
        .pointer("/params/contentChanges")
        .and_then(Value::as_array)
    else {
        return;
    };
    let Some(updated_text) = apply_content_changes(&document.text, changes) else {
        return;
    };
    if let Some(document) = state.documents.get_mut(uri) {
        document.text = updated_text;
        if incoming_version.is_some() {
            document.version = incoming_version;
        }
    }
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
        .map(|document| semantic_tokens(&document.text))
        .unwrap_or_default();
    json!({"jsonrpc":"2.0","id":id,"result":{"data":data}})
}

fn semantic_tokens(text: &str) -> Vec<u32> {
    let cancellation = AtomicBool::new(false);
    semantic_tokens_cancellable(text, &cancellation).unwrap_or_default()
}

fn semantic_tokens_cancellable(text: &str, cancellation: &AtomicBool) -> Option<Vec<u32>> {
    let mut data = Vec::new();
    for (line, current) in text.lines().enumerate() {
        if cancellation.load(Ordering::Relaxed) {
            return None;
        }
        let trimmed = current.trim_start();
        let indent = current.len().saturating_sub(trimmed.len()) as u32;
        if trimmed.starts_with("funcion ")
            || trimmed.starts_with("fn ")
            || trimmed.starts_with("estructura ")
        {
            data.extend([line as u32, indent, 2, 0, 0]);
        }
    }
    (!cancellation.load(Ordering::Relaxed)).then_some(data)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn handles_lifecycle_and_rejects_malformed_json_without_panicking() {
        let mut state = ServerState::default();
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
    fn publishes_lexer_diagnostics_after_document_open() {
        let mut state = ServerState::default();
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
    fn recovers_diagnostics_after_a_full_document_change() {
        let mut state = ServerState::default();
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
        let mut state = ServerState::default();
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
        let mut state = ServerState::default();
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
        let mut state = ServerState::default();
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
        let mut state = ServerState::default();
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
        assert_eq!(response[0]["result"]["data"], json!([0, 0, 2, 0, 0]));
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
            semantic_tokens_cancellable("funcion a():\nfuncion b():\n", &cancellation),
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
