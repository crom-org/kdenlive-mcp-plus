//! Modelo de projeto em memória: um documento MLT XML mutável
//! (`xmltree::Element`), manipulado pelas ferramentas de edição, e a sessão
//! que o mantém vivo entre chamadas MCP (com histórico de undo/redo).
//!
//! Nenhuma destas ferramentas toca o libmlt via FFI — isso continua restrito
//! a `src/engine/` (render, validação de mídia por sondagem com `ffprobe`,
//! etc.), preservando o isolamento da decisão D-003: o processo do servidor
//! MCP nunca é derrubado por uma falha de edição.

pub mod document;
pub mod profile;
pub mod session;

pub use document::ModelError;
pub use session::{Session, TrackKind};
