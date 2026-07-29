//! Sync newline-delimited JSON-RPC 2.0 over stdio.
//! Shared by the Model Context Protocol server (`peram-mcp`).
//! No tokio — matches kernel discipline; Grok/Cursor spawn us as a subprocess.

use serde_json::{json, Value};
use std::io::{BufRead, Write};

pub fn rpc_error(code: i64, message: &str) -> Value {
    json!({ "code": code, "message": message })
}

pub type DispatchResult = Result<Value, Value>;

/// Serve until stdin EOF. Notifications (no `id`) are dispatched; responses ignored.
pub fn serve_stdio<F>(mut dispatch: F) -> std::io::Result<()>
where
    F: FnMut(String, Value) -> DispatchResult,
{
    let stdin = std::io::stdin();
    let mut stdout = std::io::stdout();
    let mut lines = stdin.lock().lines();

    while let Some(line) = lines.next() {
        let line = line?;
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let msg: Value = match serde_json::from_str(trimmed) {
            Ok(v) => v,
            Err(e) => {
                let err = json!({
                    "jsonrpc": "2.0",
                    "id": Value::Null,
                    "error": rpc_error(-32700, &format!("parse error: {e}"))
                });
                writeln!(stdout, "{err}")?;
                stdout.flush()?;
                continue;
            }
        };

        let method = msg
            .get("method")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        let params = msg.get("params").cloned().unwrap_or(Value::Null);
        let id = msg.get("id").cloned();

        let Some(id) = id else {
            let _ = dispatch(method, params);
            continue;
        };

        let response = match dispatch(method, params) {
            Ok(result) => json!({ "jsonrpc": "2.0", "id": id, "result": result }),
            Err(error) => json!({ "jsonrpc": "2.0", "id": id, "error": error }),
        };
        writeln!(stdout, "{response}")?;
        stdout.flush()?;
    }
    Ok(())
}
