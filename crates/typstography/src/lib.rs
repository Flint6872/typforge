use parking_lot::Mutex;
use std::sync::Arc;
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::sync::mpsc;
use tower_lsp::LanguageServer;

use typst::World;

// Use tower_lsp's re-exported types to ensure version 0.94 consistency
pub use tower_lsp::Client;
pub use tower_lsp::lsp_types::{
    ClientCapabilities,
    ClientInfo,
    Diagnostic,
    DiagnosticSeverity,
    DidChangeTextDocumentParams,
    GotoDefinitionParams,
    Hover,
    HoverContents,
    HoverParams,
    InitializeParams,
    InitializeResult,
    MarkedString,
    MarkupContent,
    MarkupKind,
    Position,
    PublishDiagnosticsParams,
    Range,
    ServerCapabilities,
    TextDocumentContentChangeEvent,
    TextDocumentIdentifier,
    TextDocumentItem,
    TextDocumentPositionParams,
    TextDocumentSyncCapability,
    TextDocumentSyncKind,
    Url,
    VersionedTextDocumentIdentifier, // Changed Uri to Url
};

pub mod completion;
pub mod gpui_bridge;
pub mod utils;

pub trait TypstLspWorld: World + Send + Sync + 'static {
    fn update_main_source(&mut self, new_content: &str);
}

#[derive(Debug)]
pub struct TypstBackend<W: TypstLspWorld> {
    pub client: Client,
    pub world: Arc<Mutex<W>>,
    pub diagnostics_tx: mpsc::UnboundedSender<PublishDiagnosticsParams>,
}

#[tower_lsp::async_trait]
impl<W: TypstLspWorld> LanguageServer for TypstBackend<W> {
    async fn initialize(
        &self,
        _params: InitializeParams,
    ) -> tower_lsp::jsonrpc::Result<InitializeResult> {
        Ok(InitializeResult {
            capabilities: ServerCapabilities {
                text_document_sync: Some(TextDocumentSyncCapability::Kind(
                    TextDocumentSyncKind::FULL,
                )),
                hover_provider: Some(tower_lsp::lsp_types::HoverProviderCapability::Simple(true)),
                definition_provider: Some(tower_lsp::lsp_types::OneOf::Left(true)), // <--- Add this line
                completion_provider: Some(tower_lsp::lsp_types::CompletionOptions {
                    resolve_provider: Some(false),
                    trigger_characters: Some(vec!["#".to_string(), ".".to_string()]), // Common Typst triggers
                    all_commit_characters: None,
                    work_done_progress_options: Default::default(),
                    completion_item: None,
                }),
                ..Default::default()
            },
            ..Default::default()
        })
    }

    async fn shutdown(&self) -> tower_lsp::jsonrpc::Result<()> {
        Ok(())
    }

    async fn did_change(&self, params: DidChangeTextDocumentParams) {
        let uri = params.text_document.uri.clone(); // `params.text_document.uri` is already a Url
        if let Some(change) = params.content_changes.into_iter().next() {
            let mut world_guard = self.world.lock();
            world_guard.update_main_source(&change.text);
        }
        self.publish_diagnostics(uri).await;
    }

    async fn hover(
        &self,
        params: tower_lsp::lsp_types::HoverParams,
    ) -> tower_lsp::jsonrpc::Result<Option<tower_lsp::lsp_types::Hover>> {
        let position = params.text_document_position_params.position;
        // let uri = params.text_document_position_params.text_document.uri;

        // 1. Convert LSP Position (line, character) to a byte offset in Typst source.
        //    You'll need a helper function for this, similar to `byte_to_lsp_position`.
        let (_file_id, _byte_offset) = {
            let world_guard = self.world.lock();
            // This will likely involve mapping `uri` back to a `FileId` and then
            // using `Source::byte_offset(line, character)` or similar.
            // You might need to add a `uri_to_file_id` method to `TypstLspWorld`.
            // For now, let's assume `main_file_id()` if you only have one file.
            let main_file_id =
                typst::syntax::FileId::new(None, typst::syntax::VirtualPath::new("main.typ")); // Placeholder
            let source = world_guard.source(main_file_id).unwrap(); // Handle error
            let byte_offset = source
                .lines()
                .line_column_to_byte(position.line as usize, position.character as usize)
                .unwrap_or(0);
            (main_file_id, byte_offset)
        };

        // 2. Use Typst's AST/semantic analysis to get information at that offset.
        //    This is the core logic. You'll need to parse the document and traverse
        //    its AST to find the symbol at `byte_offset` and gather its information.
        let hover_content = {
            // let world_guard = self.world.lock();
            // let source = world_guard.source(file_id).unwrap(); // Already got this, but good for context

            // Example: Find a definition and get its documentation
            // This requires semantic analysis capabilities within your TypstLspWorld
            // or a Typst-specific AST/symbol resolver.
            // For a simple start, you might return "Hello, World!" on hover.
            Some("```typst\n// This is a placeholder for Typst hover info!\n```".to_string())
        };

        // 3. Format the information into `MarkedString` or `MarkupContent`.
        if let Some(content) = hover_content {
            Ok(Some(tower_lsp::lsp_types::Hover {
                contents: tower_lsp::lsp_types::HoverContents::Markup(
                    tower_lsp::lsp_types::MarkupContent {
                        kind: tower_lsp::lsp_types::MarkupKind::Markdown,
                        value: content,
                    },
                ),
                range: None, // Or the range of the hovered symbol
            }))
        } else {
            Ok(None)
        }
    }

    async fn goto_definition(
        &self,
        params: tower_lsp::lsp_types::GotoDefinitionParams,
    ) -> tower_lsp::jsonrpc::Result<Option<tower_lsp::lsp_types::GotoDefinitionResponse>> {
        let position = params.text_document_position_params.position;
        let uri = params.text_document_position_params.text_document.uri;

        // 1. Convert LSP Position to Typst byte offset (same as hover).
        let (_file_id, _byte_offset) = {
            let world_guard = self.world.lock();
            let main_file_id =
                typst::syntax::FileId::new(None, typst::syntax::VirtualPath::new("main.typ")); // Placeholder
            let source = world_guard.source(main_file_id).unwrap();
            let byte_offset = source
                .lines()
                .line_column_to_byte(position.line as usize, position.character as usize)
                .unwrap_or(0);
            (main_file_id, byte_offset)
        };

        // 2. Perform Typst-specific semantic analysis to find the definition.
        //    This is where your Typst AST/symbol resolver comes in. You need to identify
        //    what symbol is at `byte_offset` and then find its corresponding definition.
        let definition_location = {
            // Remove the `world_guard.main_file_path()` call.
            // For this placeholder, we'll use the incoming `uri` directly.
            let source_uri = uri.clone(); // Clone the uri from the request params.

            Some(tower_lsp::lsp_types::Location {
                uri: source_uri,
                range: tower_lsp::lsp_types::Range {
                    start: tower_lsp::lsp_types::Position {
                        line: 0,
                        character: 0,
                    },
                    end: tower_lsp::lsp_types::Position {
                        line: 0,
                        character: 5,
                    },
                },
            })
        };

        // 3. Return the `Location` or `LocationLink` of the definition.
        Ok(definition_location.map(tower_lsp::lsp_types::GotoDefinitionResponse::Scalar))
    }

    async fn completion(
        &self,
        params: tower_lsp::lsp_types::CompletionParams,
    ) -> tower_lsp::jsonrpc::Result<Option<tower_lsp::lsp_types::CompletionResponse>> {
        let position = params.text_document_position.position;

        let world_guard = self.world.lock();
        // Placeholder for real file ID mapping
        let main_file_id =
            typst::syntax::FileId::new(None, typst::syntax::VirtualPath::new("main.typ"));

        let source = match world_guard.source(main_file_id) {
            Ok(s) => s,
            Err(_) => return Ok(None),
        };

        let byte_offset = source
            .lines()
            .line_column_to_byte(position.line as usize, position.character as usize)
            .unwrap_or(0);

        // Access the library from the world
        let library = world_guard.library();

        let completion_items = completion::get_completions(&source, byte_offset, &library);

        Ok(Some(tower_lsp::lsp_types::CompletionResponse::Array(
            completion_items,
        )))
    }
}

impl<W: TypstLspWorld> TypstBackend<W> {
    // Changed Uri to Url
    async fn publish_diagnostics(&self, uri: Url) {
        let diagnostics_from_compile = {
            // Removed world_ptr
            let world_guard = self.world.lock();
            let output = typst::compile::<typst::layout::PagedDocument>(&*world_guard).output;
            let errs = match output {
                Ok(_) => Vec::new(),
                Err(errors) => errors.to_vec(),
            };
            // We need a reference to the world to resolve spans, but we can't hold the lock
            // while sending over the channel if it's async.
            // For simplicity in this refactor, we resolve ranges while holding the lock.
            let mut messages = Vec::new();
            for err in errs.iter() {
                let range = utils::typst_span_to_lsp_range(&*world_guard, err.span);
                messages.push(Diagnostic {
                    range,
                    severity: Some(DiagnosticSeverity::ERROR),
                    message: err.message.to_string(),
                    ..Default::default()
                });
            }
            messages // Return messages directly
        };

        let publish_params = PublishDiagnosticsParams {
            uri,
            diagnostics: diagnostics_from_compile,
            version: None,
        };
        let _ = self.diagnostics_tx.send(publish_params);
    }
}

pub async fn start_embedded_lsp<R, W, T>(
    reader: R,
    writer: W,
    shared_world: Arc<Mutex<T>>,
    diagnostics_tx_for_backend: mpsc::UnboundedSender<PublishDiagnosticsParams>,
) -> Result<(), Box<dyn std::error::Error>>
where
    R: AsyncRead + Unpin + Send + 'static,
    W: AsyncWrite + Unpin + Send + 'static,
    T: TypstLspWorld + 'static,
{
    let (service, socket) = tower_lsp::LspService::new(|client| TypstBackend {
        client,
        world: Arc::clone(&shared_world),
        diagnostics_tx: diagnostics_tx_for_backend.clone(),
    });

    tower_lsp::Server::new(reader, writer, socket)
        .serve(service)
        .await;
    Ok(())
}
