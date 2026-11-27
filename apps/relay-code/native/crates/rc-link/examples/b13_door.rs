//! Throwaway door for the B13 measurement (not committed): answers everything, one idle agent.
use futures_util::{SinkExt, StreamExt};
use serde_json::{Value, json};
use tokio_tungstenite::tungstenite::Message;

#[tokio::main]
async fn main() {
    let listener = tokio::net::TcpListener::bind("0.0.0.0:47999").await.unwrap();
    loop {
        let (tcp, peer) = listener.accept().await.unwrap();
        tokio::spawn(async move {
            let secs = || std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs_f64();
            eprintln!("{:.3} accept {peer}", secs());
            let Ok(ws) = tokio_tungstenite::accept_async(tcp).await else { return };
            let (mut tx, mut rx) = ws.split();
            let greet = json!({"relay": "remote", "v": 1, "challenge": "c", "instance": "i", "engine": "running", "host": "", "host_id": "", "version": ""});
            if tx.send(Message::Text(greet.to_string().into())).await.is_err() { return; }
            let Some(Ok(Message::Text(_hello))) = rx.next().await else { return };
            let welcome = json!({"ok": true, "host": "Test PC", "host_id": "h", "version": "t", "ops": "phone"});
            if tx.send(Message::Text(welcome.to_string().into())).await.is_err() { return; }
            while let Some(Ok(Message::Text(line))) = rx.next().await {
                let req: Value = serde_json::from_str(line.as_str()).unwrap();
                let result = match req["op"].as_str().unwrap_or("") {
                    "session.list" => json!({"sessions": [{"name": "cl", "provider": "claude", "role": "builder", "state": "idle", "project_id": 1, "branch": "b", "updated_at": "t"}]}),
                    "project.list" => json!({"projects": [{"id": 1, "name": "demo", "path": "/x"}]}),
                    _ => json!({}),
                };
                let res = json!({"v": 1, "id": req["id"], "ok": true, "result": result});
                if tx.send(Message::Text(res.to_string().into())).await.is_err() { return; }
            }
            eprintln!("{:.3} closed {peer}", secs());
        });
    }
}
