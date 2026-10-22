//! Isolamento do render em processo separado (decisão D-003): o servidor MCP
//! nunca chama o motor libmlt diretamente. Em vez disso, ele invoca o
//! próprio binário como processo filho, no subcomando `render` (ver
//! `src/main.rs`). Um crash do motor C (ex. SIGSEGV) derruba só o filho; o
//! servidor MCP continua respondendo ao agente.
//!
//! Nesta fase só existe um render por vez: quem chama `render_isolated` é
//! responsável por serializar as chamadas (o servidor MCP faz isso com um
//! `tokio::sync::Mutex`). `render_cancel`, que encerraria o processo filho de
//! verdade (diferente do original, ver docs/09-riscos-e-bugs.md), é da Fase 1
//! e não existe ainda.

use std::path::Path;
use std::process::Stdio;
use std::time::Duration;

use tokio::process::Command;

use super::EngineError;

const ENV_TIMEOUT_SECS: &str = "KDENLIVE_MCP_RENDER_TIMEOUT_SECS";
const TIMEOUT_PADRAO_SECS: u64 = 1800;

fn timeout_configurado() -> Duration {
    std::env::var(ENV_TIMEOUT_SECS)
        .ok()
        .and_then(|valor| valor.parse::<u64>().ok())
        .map(Duration::from_secs)
        .unwrap_or(Duration::from_secs(TIMEOUT_PADRAO_SECS))
}

/// Renderiza `project` para `output` com o preset `preset`, em um processo
/// filho isolado. `overwrite` controla se uma saída já existente pode ser
/// sobrescrita.
pub async fn render_isolated(
    project: &Path,
    output: &Path,
    preset: &str,
    overwrite: bool,
) -> Result<(), EngineError> {
    let exe = std::env::current_exe().map_err(|erro| {
        EngineError::InvalidText(format!("não encontrei o caminho do próprio executável: {erro}"))
    })?;

    let mut comando = Command::new(exe);
    comando
        .arg("render")
        .arg(project)
        .arg(output)
        .arg("--preset")
        .arg(preset)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        // Se o timeout expirar, a future de `wait_with_output` é cancelada e
        // o `Child` é descartado; `kill_on_drop` garante que o processo filho
        // seja morto nesse momento em vez de ficar órfão.
        .kill_on_drop(true);
    if overwrite {
        comando.arg("--overwrite");
    }

    let child = comando.spawn().map_err(|erro| {
        EngineError::InvalidText(format!("não consegui iniciar o processo de render: {erro}"))
    })?;

    let timeout = timeout_configurado();
    match tokio::time::timeout(timeout, child.wait_with_output()).await {
        Ok(Ok(saida)) => interpretar_saida(saida),
        Ok(Err(erro)) => Err(EngineError::InvalidText(format!(
            "falha ao esperar o processo de render: {erro}"
        ))),
        Err(_decorrido) => Err(EngineError::Timeout(timeout.as_secs())),
    }
}

/// Renderiza um único quadro de `project` como PNG em `output`, em processo
/// filho isolado (subcomando `frame` do próprio binário). Usada por
/// `preview_frame`.
pub async fn render_frame_isolated(
    project: &Path,
    output: &Path,
    frame: i64,
    overwrite: bool,
) -> Result<(), EngineError> {
    let exe = std::env::current_exe().map_err(|erro| {
        EngineError::InvalidText(format!("não encontrei o caminho do próprio executável: {erro}"))
    })?;

    let mut comando = Command::new(exe);
    comando
        .arg("frame")
        .arg(project)
        .arg(output)
        .arg(frame.to_string())
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    if overwrite {
        comando.arg("--overwrite");
    }

    let child = comando.spawn().map_err(|erro| {
        EngineError::InvalidText(format!("não consegui iniciar o processo de preview: {erro}"))
    })?;

    let timeout = timeout_configurado();
    match tokio::time::timeout(timeout, child.wait_with_output()).await {
        Ok(Ok(saida)) => interpretar_saida(saida),
        Ok(Err(erro)) => Err(EngineError::InvalidText(format!(
            "falha ao esperar o processo de preview: {erro}"
        ))),
        Err(_decorrido) => Err(EngineError::Timeout(timeout.as_secs())),
    }
}

fn interpretar_saida(saida: std::process::Output) -> Result<(), EngineError> {
    if saida.status.success() {
        return Ok(());
    }

    #[cfg(unix)]
    {
        use std::os::unix::process::ExitStatusExt;
        if let Some(sinal) = saida.status.signal() {
            return Err(EngineError::Signal(sinal));
        }
    }

    let codigo = saida.status.code().unwrap_or(-1);
    let stderr = String::from_utf8_lossy(&saida.stderr).into_owned();
    Err(EngineError::ChildFailed { code: codigo, stderr })
}
