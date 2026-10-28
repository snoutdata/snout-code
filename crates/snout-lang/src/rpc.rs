//! JSON-RPC 2.0 framing as LSP uses it: a `Content-Length` header, a blank line, then the
//! JSON body.

use std::io::{self, BufRead, Write};

use serde_json::Value;

/// Read one message; `None` at the end of the stream.
pub fn read_message(input: &mut impl BufRead) -> io::Result<Option<Value>> {
    let mut length: Option<usize> = None;
    let mut line = String::new();
    loop {
        line.clear();
        if input.read_line(&mut line)? == 0 {
            return Ok(None);
        }
        let header = line.trim_end_matches(['\r', '\n']);
        if header.is_empty() {
            if length.is_some() {
                break;
            }
            // Stray blank lines between messages.
            continue;
        }
        if let Some((name, value)) = header.split_once(':')
            && name.trim().eq_ignore_ascii_case("content-length")
        {
            length = value.trim().parse().ok();
        }
    }
    let length = length.unwrap_or(0);
    let mut body = vec![0; length];
    input.read_exact(&mut body)?;
    serde_json::from_slice(&body).map(Some).map_err(io::Error::other)
}

/// Write one message, framed.
pub fn write_message(output: &mut impl Write, message: &Value) -> io::Result<()> {
    let body = serde_json::to_vec(message).map_err(io::Error::other)?;
    write!(output, "Content-Length: {}\r\n\r\n", body.len())?;
    output.write_all(&body)?;
    output.flush()
}

/// A JSON-RPC error, as a response carries it.
#[derive(Clone, Debug, PartialEq)]
pub struct ResponseError {
    pub code: i64,
    pub message: String,
}

impl std::fmt::Display for ResponseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} ({})", self.message, self.code)
    }
}

impl std::error::Error for ResponseError {}

impl ResponseError {
    /// The server has gone (or was never there).
    pub const SERVER_GONE: i64 = -32099;
    /// The request was cancelled.
    pub const CANCELLED: i64 = -32800;

    pub fn gone() -> Self {
        Self { code: Self::SERVER_GONE, message: "the language server is not running".into() }
    }

    pub fn of(value: &Value) -> Self {
        Self {
            code: value.get("code").and_then(Value::as_i64).unwrap_or(-32603),
            message: value.get("message").and_then(Value::as_str).unwrap_or("unknown error").to_owned(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn frames_round_trip_including_multibyte_bodies() {
        let mut buf = Vec::new();
        let a = json!({"jsonrpc": "2.0", "id": 1, "method": "x", "params": {"text": "naïve ☕"}});
        let b = json!({"jsonrpc": "2.0", "method": "y"});
        write_message(&mut buf, &a).unwrap();
        write_message(&mut buf, &b).unwrap();
        let mut input = io::Cursor::new(buf);
        assert_eq!(read_message(&mut input).unwrap(), Some(a));
        assert_eq!(read_message(&mut input).unwrap(), Some(b));
        assert_eq!(read_message(&mut input).unwrap(), None);
    }

    #[test]
    fn reads_headers_in_any_case_with_extra_headers() {
        let body = r#"{"id":3}"#;
        let text = format!(
            "content-type: application/vscode-jsonrpc; charset=utf-8\r\nCONTENT-LENGTH: {}\r\n\r\n{body}",
            body.len()
        );
        let mut input = io::Cursor::new(text.into_bytes());
        assert_eq!(read_message(&mut input).unwrap(), Some(json!({"id": 3})));
    }
}
