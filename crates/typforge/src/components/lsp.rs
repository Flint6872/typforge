// use std::sync::Arc;
// use std::sync::atomic::{AtomicU64, Ordering};
// use tokio::io::{AsyncReadExt, AsyncWriteExt, duplex};
// use tokio::sync::mpsc;
// // Import everything from typstography/tower_lsp::lsp_types (0.94)
// use typstography::{
//     ClientCapabilities, ClientInfo, DidChangeTextDocumentParams, GotoDefinitionParams, HoverParams,
//     InitializeParams, Position, PublishDiagnosticsParams, TextDocumentContentChangeEvent,
//     TextDocumentIdentifier, TextDocumentPositionParams, TypstLspWorld, Url,
//     VersionedTextDocumentIdentifier,
// };

// pub struct LspClient {
//     server_input_tx: mpsc::UnboundedSender<String>,
//     next_request_id: Arc<AtomicU64>,
// }

// impl LspClient {
//     pub fn new<W: TypstLspWorld + 'static>(
//         world: Arc<parking_lot::Mutex<W>>,
//     ) -> (
//         Self,
//         mpsc::UnboundedReceiver<PublishDiagnosticsParams>,
//         mpsc::UnboundedReceiver<serde_json::Value>,
//     ) {
//         let (server_reader, mut client_writer) = duplex(1024 * 16);
//         let (mut client_reader, server_writer) = duplex(1024 * 16);

//         let (diagnostics_tx, diagnostics_rx) = mpsc::unbounded_channel();
//         let (responses_tx, responses_rx) = mpsc::unbounded_channel();
//         let (server_input_tx, mut server_input_rx) = mpsc::unbounded_channel::<String>();

//         let world_for_server = world.clone();

//         tokio::spawn(async move {
//             typstography::start_embedded_lsp(
//                 server_reader,
//                 server_writer,
//                 world_for_server,
//                 diagnostics_tx,
//             )
//             .await
//             .expect("LSP server crashed");
//         });

//         tokio::spawn(async move {
//             while let Some(msg) = server_input_rx.recv().await {
//                 let framed = format!("Content-Length: {}\r\n\r\n{}", msg.len(), msg);
//                 let _ = client_writer.write_all(framed.as_bytes()).await;
//             }
//         });

//         tokio::spawn(async move {
//             let mut buffer = Vec::new();
//             let mut temp_buf = [0u8; 1024];
//             loop {
//                 let n = client_reader.read(&mut temp_buf).await.unwrap_or(0);
//                 if n == 0 {
//                     break;
//                 }
//                 buffer.extend_from_slice(&temp_buf[..n]);

//                 // Very simple LSP frame parser
//                 let content_str = String::from_utf8_lossy(&buffer);
//                 if let Some(start_idx) = content_str.find("\r\n\r\n") {
//                     let header = &content_str[..start_idx];
//                     if let Some(len_str) = header.split("Content-Length: ").nth(1) {
//                         if let Ok(content_len) =
//                             len_str.split("\r\n").next().unwrap_or("").parse::<usize>()
//                         {
//                             let total_needed = start_idx + 4 + content_len;
//                             if buffer.len() >= total_needed {
//                                 let body = &buffer[start_idx + 4..total_needed];
//                                 if let Ok(json) = serde_json::from_slice::<serde_json::Value>(body)
//                                 {
//                                     let _ = responses_tx.send(json);
//                                 }
//                                 buffer.drain(..total_needed);
//                             }
//                         }
//                     }
//                 }
//             }
//         });

//         (
//             Self {
//                 server_input_tx,
//                 next_request_id: Arc::new(AtomicU64::new(1)),
//             },
//             diagnostics_rx,
//             responses_rx,
//         )
//     }

//     /// Helper to send a generic LSP request and return its generated ID.
//     fn send_request<P: serde::Serialize>(&self, method: &str, params: P) -> Option<u64> {
//         let id = self.next_request_id.fetch_add(1, Ordering::SeqCst);
//         if let Ok(json) = serde_json::to_string(&serde_json::json!({
//             "jsonrpc": "2.0",
//             "id": id,
//             "method": method,
//             "params": params
//         })) {
//             let _ = self.server_input_tx.send(json);
//             Some(id)
//         } else {
//             None
//         }
//     }

//     /// Helper to send a generic LSP notification.
//     fn send_notification<P: serde::Serialize>(&self, method: &str, params: P) {
//         if let Ok(json) = serde_json::to_string(&serde_json::json!({
//             "jsonrpc": "2.0",
//             "method": method,
//             "params": params
//         })) {
//             let _ = self.server_input_tx.send(json);
//         }
//     }

//     pub fn initialize(&self, client_capabilities: ClientCapabilities) {
//         let params = InitializeParams {
//             process_id: Some(std::process::id()),
//             client_info: Some(ClientInfo {
//                 name: "Typforge".to_string(),
//                 version: Some("0.0.1".to_string()),
//             }),
//             capabilities: client_capabilities,
//             locale: None,
//             root_path: None,
//             root_uri: None,
//             initialization_options: None,
//             trace: None,
//             workspace_folders: None,
//         };

//         self.send_request("initialize", params);
//     }

//     pub fn did_change(&self, uri: Url, text: String, version: i32) {
//         let params = DidChangeTextDocumentParams {
//             text_document: VersionedTextDocumentIdentifier { uri, version },
//             content_changes: vec![TextDocumentContentChangeEvent {
//                 range: None,
//                 range_length: None,
//                 text,
//             }],
//         };

//         self.send_notification("textDocument/didChange", params);
//     }

//     pub fn hover(&self, uri: Url, position: Position) -> Option<u64> {
//         let params = HoverParams {
//             text_document_position_params: TextDocumentPositionParams {
//                 text_document: TextDocumentIdentifier { uri },
//                 position,
//             },
//             work_done_progress_params: Default::default(),
//         };
//         self.send_request("textDocument/hover", params)
//     }

//     pub fn goto_definition(&self, uri: Url, position: Position) -> Option<u64> {
//         let params = GotoDefinitionParams {
//             text_document_position_params: TextDocumentPositionParams {
//                 text_document: TextDocumentIdentifier { uri },
//                 position,
//             },
//             work_done_progress_params: Default::default(),
//             partial_result_params: Default::default(),
//         };
//         self.send_request("textDocument/definition", params)
//     }
// }
