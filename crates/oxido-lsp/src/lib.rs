#![forbid(unsafe_code)]

use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    io::{self, BufRead},
};

pub const MAX_MESSAGE_BYTES: usize = 4 * 1024 * 1024;
const MAX_HEADER_LINE_BYTES: usize = 8 * 1024;

#[derive(Default)]
pub struct ServerState {
    documents: BTreeMap<String, String>,
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
        "initialize" => id.map(|id| json!({"jsonrpc":"2.0","id":id,"result":{"capabilities":{"textDocumentSync":1,"semanticTokensProvider":{"full":true,"legend":{"tokenTypes":["keyword"],"tokenModifiers":[]}}}}})),
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
        && let Some(text) = state.documents.get(uri)
    {
        messages.push(diagnostic_notification(uri, text));
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
    state.documents.insert(uri.to_owned(), text.to_owned());
}

fn change_document(request: &Value, state: &mut ServerState) {
    let (Some(uri), Some(text)) = (
        request
            .pointer("/params/textDocument/uri")
            .and_then(Value::as_str),
        request
            .pointer("/params/contentChanges/0/text")
            .and_then(Value::as_str),
    ) else {
        return;
    };
    state.documents.insert(uri.to_owned(), text.to_owned());
}

fn semantic_response(id: Value, params: Option<&Value>, state: &ServerState) -> Value {
    let uri = params
        .and_then(|value| value.pointer("/textDocument/uri"))
        .and_then(Value::as_str);
    let data = uri
        .and_then(|uri| state.documents.get(uri))
        .map(|text| semantic_tokens(text))
        .unwrap_or_default();
    json!({"jsonrpc":"2.0","id":id,"result":{"data":data}})
}

fn semantic_tokens(text: &str) -> Vec<u32> {
    let mut data = Vec::new();
    for (line, current) in text.lines().enumerate() {
        let trimmed = current.trim_start();
        let indent = current.len().saturating_sub(trimmed.len()) as u32;
        if trimmed.starts_with("funcion ")
            || trimmed.starts_with("fn ")
            || trimmed.starts_with("estructura ")
        {
            data.extend([line as u32, indent, 2, 0, 0]);
        }
    }
    data
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
        assert_eq!(result[0]["result"]["capabilities"]["textDocumentSync"], 1);
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
