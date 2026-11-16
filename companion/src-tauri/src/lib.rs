//! Texure Companion Tauri Backend.
//!
//! Provides the outbound reverse WebSocket tunnel engine, local loopback proxy router,
//! and Tauri IPC bridge connecting the React frontend with Android bridge infrastructure.

pub mod commands;
pub mod proxy;
pub mod tunnel;

pub use commands::AppState;
pub use proxy::ProxyRouter;
pub use tunnel::{ReverseTunnel, TunnelConfig, TunnelHandle, TunnelState};

pub use texure_contract as contract;
pub use texure_contract::{
    ErrorCode, JsonRpcError, JsonRpcRequest, JsonRpcResponse, ResolvedDeviceConfig, TapRequest,
    TexureConfig, TypeRequest, UiNode, UiTree,
};

#[cfg(feature = "tauri-app")]
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .manage(AppState::new())
        .invoke_handler(tauri::generate_handler![
            commands::start_tunnel,
            commands::stop_tunnel,
            commands::get_tunnel_status,
            commands::get_target_status,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[cfg(not(feature = "tauri-app"))]
pub fn run() {
    eprintln!("Texure Companion running in headless/core mode (tauri-app feature disabled).");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_core_types_integration() {
        // Verify wire types and error codes from texure-contract
        let err = ErrorCode::Offline;
        assert_eq!(err.as_str(), "OFFLINE");

        let req = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: Some(serde_json::json!(1)),
            method: "ui_tree".to_string(),
            params: None,
        };
        let serialized = serde_json::to_string(&req).expect("serialize jsonrpc request");
        assert!(serialized.contains("ui_tree"));

        let tap = TapRequest::new(42).with_index(5);
        assert_eq!(tap.rev, 42);
        assert_eq!(tap.index, Some(5));
    }
}
