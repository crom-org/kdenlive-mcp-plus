//! Motor de render: chamada direta à API C do libmlt (`ffi`), resolução de
//! presets (`preset`) e execução do render (`render`). O isolamento em
//! processo separado mora em `worker`.

mod ffi;
pub mod jobs;
pub mod preset;
pub mod queue;
pub mod render;
pub mod worker;

use std::path::PathBuf;

pub use preset::RenderOptions;

/// Erros do motor de render. Toda mensagem é em português e diz o que
/// aconteceu; quando faz sentido, também diz o que fazer.
#[derive(Debug, thiserror::Error)]
pub enum EngineError {
    #[error("projeto não encontrado: {0}")]
    ProjectNotFound(PathBuf),

    #[error("o arquivo de saída já existe: {0} (use overwrite=true para sobrescrever)")]
    OutputExists(PathBuf),

    #[error("{0}")]
    InvalidText(String),

    #[error("falha ao criar {0} no libmlt")]
    Create(&'static str),

    #[error("falha ao conectar o consumidor de render ao produtor do projeto")]
    Connect,

    #[error("falha ao iniciar o consumidor de render")]
    Start,

    #[error("o render terminou mas o arquivo de saída não existe ou está vazio: {0}")]
    OutputMissing(PathBuf),

    #[error(
        "o render excedeu o tempo limite de {0}s e foi encerrado (ajuste com a variável de \
         ambiente KDENLIVE_MCP_RENDER_TIMEOUT_SECS)"
    )]
    Timeout(u64),

    #[error("o processo de render terminou com código {code}. Saída de erro: {stderr}")]
    ChildFailed { code: i32, stderr: String },

    #[error(
        "o motor de render travou (sinal {0}). Isso costuma indicar um projeto XML malformado \
         ou um codec indisponível; o servidor MCP continua respondendo normalmente"
    )]
    Signal(i32),
}
