use anyhow::{Context, Result, bail};
use serde_json::{Value, json};
use tungstenite::{Message, connect};

pub const DEFAULT_PORT: u16 = 8080;

/// Finds the WebSocket URL of Steam's SharedJSContext, where the library stores live.
pub fn shared_context_url(port: u16) -> Result<String> {
    let url = format!("http://127.0.0.1:{port}/json");
    let targets: Vec<Value> = ureq::get(&url)
        .call()
        .with_context(|| format!("Steam debug port {port} is not reachable (is the flag file present and Steam restarted?)"))?
        .body_mut()
        .read_json()?;

    targets
        .iter()
        .find(|t| t["title"] == "SharedJSContext")
        .and_then(|t| t["webSocketDebuggerUrl"].as_str())
        .map(str::to_owned)
        .context("SharedJSContext target not found")
}

/// Evaluates a JavaScript expression in Steam's library context and returns its value.
pub fn eval(ws_url: &str, expression: &str) -> Result<Value> {
    let (mut socket, _) = connect(ws_url).context("connecting to Steam debug socket")?;
    let request = json!({
        "id": 1,
        "method": "Runtime.evaluate",
        "params": { "expression": expression, "returnByValue": true, "awaitPromise": true }
    });
    socket.send(Message::text(request.to_string()))?;

    loop {
        let Message::Text(text) = socket.read()? else { continue };
        let reply: Value = serde_json::from_str(text.as_str())?;
        if reply["id"] != 1 {
            continue;
        }
        if let Some(err) = reply.get("error") {
            bail!("CDP error: {err}");
        }
        let result = &reply["result"];
        if let Some(exc) = result.get("exceptionDetails") {
            bail!("JS exception: {exc}");
        }
        return Ok(result["result"]["value"].clone());
    }
}
