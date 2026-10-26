#![forbid(unsafe_code)]

use serde_json::{Value, json};
use std::{collections::BTreeMap, io};

pub const MAX_MESSAGE_BYTES: usize = 4 * 1024 * 1024;

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
}
