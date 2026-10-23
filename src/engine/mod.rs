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

/// Encerra o processo **sem** rodar os destrutores nem os `atexit`.
///
/// Só para os subprocessos de render (`render` e `frame`), depois que o
/// arquivo de saída já foi gravado e o consumidor fechado. Motivo: o
/// `mlt_factory_init` registra um `atexit` (`mlt_factory_close`) que
/// descarrega os módulos do MLT; se uma thread de render ainda estiver
/// dentro de um deles (visto com o módulo Qt, em projetos de 1080x1920,
/// confirmado com o gdb: `exit` → `mlt_factory_close` → `dlclose` enquanto
/// outra thread estava em `QPainter`), o processo morre com SIGSEGV **depois**
/// de ter feito o trabalho — e o servidor, vendo um sinal, devolvia "o motor
/// travou" para um render que tinha dado certo. O sistema operacional libera
/// a memória e as threads do processo.
pub fn encerrar_sem_limpeza(codigo: i32) -> ! {
    use std::io::Write;
    let _ = std::io::stdout().flush();
    let _ = std::io::stderr().flush();
    // SAFETY: `_exit` não retorna e não lê nenhuma memória do programa; os
    // buffers de saída já foram descarregados acima. Nada além do próprio
    // processo é afetado.
    unsafe { libc::_exit(codigo) }
}

/// Variáveis de ambiente que um subprocesso de render precisa para o módulo
/// Qt do libmlt (títulos `kdenlivetitle`, o filtro/transição `qtblend`) funcionar
/// **sem tela**. O libmlt recusa o módulo se não houver `DISPLAY` nem
/// `WAYLAND_DISPLAY` — e os títulos saem como o texto "INVALID" — mas basta
/// o `DISPLAY` existir: com `QT_QPA_PLATFORM=offscreen` o Qt ignora o valor e
/// desenha na memória. Um cliente MCP como o Claude Desktop inicia o servidor
/// sem `DISPLAY` (só com `HOME`, `PATH`, `USER`, `LOGNAME` e `SHELL`), então
/// sem isto o render de qualquer projeto com título ou composição sairia errado.
/// Com uma tela de verdade (`DISPLAY`/`WAYLAND_DISPLAY`), não muda nada.
pub fn ambiente_sem_tela() -> Vec<(&'static str, &'static str)> {
    ambiente_sem_tela_para(
        std::env::var_os("DISPLAY").is_some(),
        std::env::var_os("WAYLAND_DISPLAY").is_some(),
        std::env::var_os("QT_QPA_PLATFORM").is_some(),
    )
}

fn ambiente_sem_tela_para(tem_display: bool, tem_wayland: bool, tem_plataforma_qt: bool) -> Vec<(&'static str, &'static str)> {
    if tem_display || tem_wayland {
        return Vec::new();
    }
    let mut variaveis = vec![("DISPLAY", ":99")];
    if !tem_plataforma_qt {
        variaveis.push(("QT_QPA_PLATFORM", "offscreen"));
    }
    variaveis
}

#[cfg(test)]
mod testes_ambiente {
    use super::ambiente_sem_tela_para;

    #[test]
    fn com_tela_nao_muda_nada_e_sem_tela_liga_o_qt_offscreen() {
        assert!(ambiente_sem_tela_para(true, false, false).is_empty());
        assert!(ambiente_sem_tela_para(false, true, false).is_empty());
        assert_eq!(
            ambiente_sem_tela_para(false, false, false),
            vec![("DISPLAY", ":99"), ("QT_QPA_PLATFORM", "offscreen")]
        );
        // quem já escolheu a plataforma do Qt é respeitado
        assert_eq!(ambiente_sem_tela_para(false, false, true), vec![("DISPLAY", ":99")]);
    }
}
