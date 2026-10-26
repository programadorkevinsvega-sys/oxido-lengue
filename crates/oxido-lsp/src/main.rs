#![forbid(unsafe_code)]

use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    io::{self, BufRead, Read, Write},
};

struct ServerState {
    documents: BTreeMap<String, String>,
}

fn main() -> io::Result<()> {
    let stdin = io::stdin();
    let mut input = stdin.lock();
    let stdout = io::stdout();
    let mut output = stdout.lock();
    let mut state = ServerState {
        documents: BTreeMap::new(),
    };
    loop {
        let mut length = None;
        let mut line = String::new();
        loop {
            line.clear();
            if input.read_line(&mut line)? == 0 {
                return Ok(());
            }
            if line == "\r\n" || line == "\n" {
                break;
            }
            if let Some(value) = line.strip_prefix("Content-Length:") {
                length = value.trim().parse::<usize>().ok();
            }
        }
        let length = length
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "falta Content-Length"))?;
        let mut body = vec![0_u8; length];
        input.read_exact(&mut body)?;
        let request: Value = serde_json::from_slice(&body).map_err(io::Error::other)?;
        for response in handle(request, &mut state) {
            let encoded = serde_json::to_vec(&response).map_err(io::Error::other)?;
            write!(output, "Content-Length: {}\r\n\r\n", encoded.len())?;
            output.write_all(&encoded)?;
            output.flush()?;
        }
    }
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
