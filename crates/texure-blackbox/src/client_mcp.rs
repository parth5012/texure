use serde_json::Value;
use std::process::Stdio;
use texure_contract::rpc::{JsonRpcRequest, JsonRpcResponse};
use thiserror::Error;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, ChildStdin, ChildStdout, Command};

#[derive(Error, Debug)]
pub enum McpClientError {
    #[error("Failed to spawn process {command}: {source}")]
    SpawnError {
        command: String,
        source: std::io::Error,
    },
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("JSON-RPC serialize/deserialize error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("MCP server error {code}: {message}")]
    ServerError { code: i64, message: String },
    #[error("No response from server")]
    NoResponse,
    #[error("Child process terminated")]
    Terminated,
}

pub struct McpStdioClient {
    _child: Child,
    stdin: ChildStdin,
    stdout_lines: tokio::io::Lines<BufReader<ChildStdout>>,
    next_id: u64,
}

impl McpStdioClient {
    pub fn spawn(command: &str, args: &[&str]) -> Result<Self, McpClientError> {
        let mut child = Command::new(command)
            .args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .kill_on_drop(true)
            .spawn()
            .map_err(|e| McpClientError::SpawnError {
                command: command.to_string(),
                source: e,
            })?;

        let stdin = child.stdin.take().expect("failed to take child stdin");
        let stdout = child.stdout.take().expect("failed to take child stdout");
        let stdout_lines = BufReader::new(stdout).lines();

        Ok(Self {
            _child: child,
            stdin,
            stdout_lines,
            next_id: 1,
        })
    }

    pub async fn send_request(
        &mut self,
        method: &str,
        params: Option<Value>,
    ) -> Result<Value, McpClientError> {
        let id = self.next_id;
        self.next_id += 1;

        let req = JsonRpcRequest::with_numeric_id(id, method, params);

        let mut raw = serde_json::to_string(&req)?;
        raw.push('\n');
        self.stdin.write_all(raw.as_bytes()).await?;
        self.stdin.flush().await?;

        // Read response line
        while let Some(line) = self.stdout_lines.next_line().await? {
            if line.trim().is_empty() {
                continue;
            }
            let resp: JsonRpcResponse = match serde_json::from_str(&line) {
                Ok(resp) => resp,
                Err(_) => continue,
            };
            if resp.matches_numeric_id(id) {
                if let Some(err) = resp.error {
                    return Err(McpClientError::ServerError {
                        code: err.code,
                        message: err.message,
                    });
                }
                return resp.result.ok_or(McpClientError::NoResponse);
            } else if resp.id.is_none() {
                if let Some(err) = resp.error {
                    return Err(McpClientError::ServerError {
                        code: err.code,
                        message: err.message,
                    });
                }
            }
        }

        Err(McpClientError::Terminated)
    }

    pub async fn initialize(&mut self) -> Result<Value, McpClientError> {
        self.send_request(
            "initialize",
            Some(serde_json::json!({
                "protocolVersion": "2024-11-05",
                "capabilities": {},
                "clientInfo": {
                    "name": "texure-blackbox",
                    "version": "0.1.0"
                }
            })),
        )
        .await
    }

    pub async fn list_tools(&mut self) -> Result<Value, McpClientError> {
        self.send_request("tools/list", None).await
    }

    pub async fn call_tool(
        &mut self,
        name: &str,
        arguments: Value,
    ) -> Result<Value, McpClientError> {
        self.send_request(
            "tools/call",
            Some(serde_json::json!({
                "name": name,
                "arguments": arguments
            })),
        )
        .await
    }
}
