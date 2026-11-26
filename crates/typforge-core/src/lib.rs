pub mod compiler;
pub mod diagnostics;
pub mod session;
pub mod world;

pub use compiler::{CompileOutput, CompileRequest, CompilerHost};
pub use diagnostics::{ResolvedDiagnostic, resolve_diagnostics};
pub use session::{DocumentSession, EditOrigin};
pub use world::ProjectWorld;

use std::sync::Arc;
use typst::diag::SourceDiagnostic;
use typst_layout::PagedDocument;

/// Compilation results returned by `typforge-core`.
#[derive(Debug, Clone)]
pub struct CompileResult {
    pub document: Option<Arc<PagedDocument>>,
    pub warnings: Vec<SourceDiagnostic>,
    pub errors: Vec<SourceDiagnostic>,
}

impl CompileResult {
    pub fn is_ok(&self) -> bool {
        self.document.is_some() && self.errors.is_empty()
    }
}

/// Headless compile function free from GPUI dependencies.
pub fn compile(world: &ProjectWorld) -> CompileResult {
    let output = typst::compile(world);
    let warnings = output.warnings.into_iter().collect();

    match output.output {
        Ok(doc) => CompileResult {
            document: Some(Arc::new(doc)),
            warnings,
            errors: Vec::new(),
        },
        Err(errors) => CompileResult {
            document: None,
            warnings,
            errors: errors.into_iter().collect(),
        },
    }
}
