//! `kdenlive_live`, `kdenlive_reload`, `kdenlive_open` (D-060): acompanhar a
//! edição no Kdenlive aberto sem depender de variável de ambiente na hora de
//! iniciar o servidor. Usa o mesmo mecanismo do D-042 (File > Revert pelo
//! D-Bus, `mcp::live`).

use std::path::{Path, PathBuf};
use std::sync::atomic::Ordering;

use serde_json::{Value, json};

use crate::mcp::live::{self, Resultado};
use crate::mcp::params::{KdenliveLiveParams, KdenliveOpenParams};
use crate::mcp::server::{KdenliveMcpServer, erro};

/// Programa do Kdenlive: `KDENLIVE_MCP_KDENLIVE`, senão `kdenlive` no `PATH`,
/// senão o AppImage mais recente em `~/Applications` (`kdenlive*.AppImage`).
fn programa_kdenlive() -> Option<PathBuf> {
    if let Some(p) = std::env::var_os("KDENLIVE_MCP_KDENLIVE") {
        return Some(PathBuf::from(p));
    }
    if let Some(caminhos) = std::env::var_os("PATH") {
        for dir in std::env::split_paths(&caminhos) {
            let candidato = dir.join("kdenlive");
            if candidato.is_file() {
                return Some(candidato);
            }
        }
    }
    let apps = PathBuf::from(std::env::var_os("HOME")?).join("Applications");
    let mut appimages: Vec<(std::time::SystemTime, PathBuf)> = std::fs::read_dir(apps)
        .ok()?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| {
            p.file_name().and_then(|n| n.to_str()).is_some_and(|n| {
                let n = n.to_lowercase();
                n.starts_with("kdenlive") && n.ends_with(".appimage")
            })
        })
        .filter_map(|p| Some((p.metadata().ok()?.modified().ok()?, p)))
        .collect();
    appimages.sort();
    appimages.pop().map(|(_, p)| p)
}

fn resultado_json(r: &Resultado) -> &'static str {
    match r {
        Resultado::Recarregado(_) => "recarregado",
        Resultado::NaoEncontrado => "kdenlive_com_o_projeto_nao_encontrado",
        Resultado::ComAlteracoesNaoSalvas => "kdenlive_tem_alteracoes_nao_salvas",
        Resultado::Ocupado => "kdenlive_ocupado",
    }
}

impl KdenliveMcpServer {
    /// Caminho do projeto desta sessão (erro se não houver projeto ou se ele
    /// nunca foi salvo).
    async fn caminho_do_projeto(&self) -> Result<PathBuf, Value> {
        self.with_session(|s| s.path.clone())
            .await?
            .ok_or_else(|| erro("o projeto ainda não foi salvo: chame project_save com um caminho primeiro"))
    }

    pub(crate) async fn kdenlive_live_impl(&self, params: KdenliveLiveParams) -> Value {
        if params.enabled {
            self.ligar_recarga();
            self.autosave.store(true, Ordering::SeqCst);
            // grava e recarrega já, para o Kdenlive mostrar o estado atual
            let salvo = {
                let mut guard = self.session.lock().await;
                match guard.as_mut() {
                    Some(s) if s.path.is_some() => s.save(None).ok(),
                    _ => None,
                }
            };
            if let Some(caminho) = &salvo {
                self.pedir_recarga(caminho);
            }
            json!({
                "ok": true,
                "ao_vivo": true,
                "projeto": salvo.map(|p| p.display().to_string()),
                "nota": "cada edição regrava o projeto e o Kdenlive que o mostra recarrega sozinho (se não tiver alterações não salvas)",
            })
        } else {
            self.autosave.store(false, Ordering::SeqCst);
            self.desligar_recarga();
            json!({ "ok": true, "ao_vivo": false })
        }
    }

    pub(crate) async fn kdenlive_reload_impl(&self) -> Value {
        let caminho = match self.caminho_do_projeto().await {
            Ok(c) => c,
            Err(e) => return e,
        };
        {
            let mut guard = self.session.lock().await;
            if let Some(s) = guard.as_mut()
                && let Err(e) = s.save(None)
            {
                return erro(format!("não consegui gravar o projeto antes de recarregar: {e}"));
            }
        }
        match live::recarregar(&caminho).await {
            Ok(r) => json!({
                "ok": true,
                "resultado": resultado_json(&r),
                "projeto": caminho.display().to_string(),
            }),
            Err(e) => erro(format!("não consegui falar com o Kdenlive pelo D-Bus: {e}")),
        }
    }

    pub(crate) async fn kdenlive_open_impl(&self, params: KdenliveOpenParams) -> Value {
        let caminho = match params.path {
            Some(p) => PathBuf::from(p),
            None => match self.caminho_do_projeto().await {
                Ok(c) => c,
                Err(e) => return e,
            },
        };
        if !caminho.is_file() {
            return erro(format!("projeto não encontrado: {}", caminho.display()));
        }
        if let Ok((Some(conexao), _)) = live::conexao_do_projeto(&caminho).await {
            return json!({ "ok": true, "ja_aberto": true, "conexao": conexao, "projeto": caminho.display().to_string() });
        }
        let Some(programa) = programa_kdenlive() else {
            return erro(
                "Kdenlive não encontrado: defina KDENLIVE_MCP_KDENLIVE, ponha `kdenlive` no PATH ou um kdenlive*.AppImage em ~/Applications",
            );
        };
        match abrir(&programa, &caminho) {
            Ok(pid) => json!({
                "ok": true,
                "ja_aberto": false,
                "pid": pid,
                "programa": programa.display().to_string(),
                "projeto": caminho.display().to_string(),
                "nota": "o Kdenlive leva alguns segundos para abrir; use kdenlive_live para acompanhar as edições",
            }),
            Err(e) => erro(format!("não consegui abrir o Kdenlive ({}): {e}", programa.display())),
        }
    }
}

/// Abre o Kdenlive com o projeto, desligado deste processo (grupo próprio,
/// sem stdin/stdout — o stdout do modo `mcp` é só do protocolo).
fn abrir(programa: &Path, projeto: &Path) -> std::io::Result<u32> {
    use std::os::unix::process::CommandExt;
    let mut comando = std::process::Command::new(programa);
    comando
        .arg(projeto)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .process_group(0);
    for (k, v) in live::ambiente_para_abrir_kdenlive() {
        comando.env(k, v);
    }
    let mut filho = comando.spawn()?;
    let pid = filho.id();
    // recolhe o processo quando ele terminar, sem segurar o servidor
    std::thread::spawn(move || {
        let _ = filho.wait();
    });
    Ok(pid)
}
