//! Registro de renders assíncronos, para `render_progress`/`render_cancel`
//! (Fase 1) encerrarem/consultarem um render de verdade — diferente do
//! `render_queue_stop` do projeto original, que só marcava um estado sem
//! encerrar nada (ver docs/09-riscos-e-bugs.md).
//!
//! Continua tudo isolado em processo filho (D-003): este módulo só
//! supervisiona o processo, nunca chama o libmlt diretamente.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, AtomicU64, AtomicU8, Ordering};
use std::sync::Arc;

use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use tokio::sync::Mutex;

use super::EngineError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum JobStatus {
    /// Aguardando sua vez (outro render está ocupando o `render_lock`
    /// compartilhado — só um render de verdade por vez, decisão D-016).
    /// O processo filho ainda não foi criado; não há pid.
    Queued,
    Running,
    Done,
    Failed(String),
    Cancelled,
}

impl JobStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            JobStatus::Queued => "na fila",
            JobStatus::Running => "em andamento",
            JobStatus::Done => "concluído",
            JobStatus::Failed(_) => "falhou",
            JobStatus::Cancelled => "cancelado",
        }
    }
}

pub struct RenderJob {
    pub id: String,
    pub output_path: PathBuf,
    percent: AtomicU8,
    pid: AtomicU32,
    status: Mutex<JobStatus>,
}

impl RenderJob {
    pub async fn status(&self) -> JobStatus {
        self.status.lock().await.clone()
    }

    pub fn percent(&self) -> u8 {
        self.percent.load(Ordering::Relaxed)
    }
}

/// Registro em memória dos renders assíncronos deste processo. Não
/// sobrevive a um reinício do servidor (fila simples, mesma decisão da
/// Fase 0, D-016).
#[derive(Clone, Default)]
pub struct JobRegistry {
    jobs: Arc<Mutex<HashMap<String, Arc<RenderJob>>>>,
    next_id: Arc<AtomicU64>,
}

impl JobRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// Registra um render para rodar assim que o `render_lock` compartilhado
    /// estiver livre (um render de verdade por vez no processo, decisão
    /// D-016 — vale tanto para `export_render(wait=false)` quanto para a
    /// fila `render_queue_*`, que passam o mesmo lock usado por
    /// `export_render(wait=true)`). Devolve imediatamente o `RenderJob`,
    /// com status `Queued` até a vez dele chegar; o processo filho só é
    /// criado depois disso, numa tarefa de fundo.
    pub async fn spawn(
        &self,
        render_lock: Arc<Mutex<()>>,
        project: &Path,
        output: &Path,
        preset: &str,
        overwrite: bool,
    ) -> Result<Arc<RenderJob>, EngineError> {
        let exe = std::env::current_exe().map_err(|erro| {
            EngineError::InvalidText(format!("não encontrei o caminho do próprio executável: {erro}"))
        })?;

        let job_id = format!("job{}", self.next_id.fetch_add(1, Ordering::Relaxed));
        let job = Arc::new(RenderJob {
            id: job_id.clone(),
            output_path: output.to_path_buf(),
            percent: AtomicU8::new(0),
            pid: AtomicU32::new(0),
            status: Mutex::new(JobStatus::Queued),
        });
        self.jobs.lock().await.insert(job_id, job.clone());

        let project = project.to_path_buf();
        let output_buf = output.to_path_buf();
        let preset = preset.to_string();
        let job_bg = job.clone();

        tokio::spawn(async move {
            let _guarda_render = render_lock.lock().await;

            {
                let mut status = job_bg.status.lock().await;
                if *status == JobStatus::Cancelled {
                    // Foi cancelado enquanto esperava a vez; nunca chega a
                    // criar o processo filho.
                    return;
                }
                *status = JobStatus::Running;
            }

            let mut comando = Command::new(exe);
            comando
                .arg("render")
                .arg(&project)
                .arg(&output_buf)
                .arg("--preset")
                .arg(&preset)
                .envs(crate::engine::ambiente_sem_tela())
                .stdin(std::process::Stdio::null())
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::piped())
                .kill_on_drop(true);
            if overwrite {
                comando.arg("--overwrite");
            }

            let mut child = match comando.spawn() {
                Ok(c) => c,
                Err(erro) => {
                    *job_bg.status.lock().await = JobStatus::Failed(format!(
                        "não consegui iniciar o processo de render: {erro}"
                    ));
                    return;
                }
            };
            job_bg.pid.store(child.id().unwrap_or(0), Ordering::Relaxed);

            let stderr = child
                .stderr
                .take()
                .expect("stderr foi configurado como piped ao criar o comando");
            let mut linhas = BufReader::new(stderr).lines();
            let mut stderr_completo = String::new();
            while let Ok(Some(linha)) = linhas.next_line().await {
                if let Some(pct) = parse_percentage(&linha) {
                    job_bg.percent.store(pct, Ordering::Relaxed);
                }
                stderr_completo.push_str(&linha);
                stderr_completo.push('\n');
            }

            let resultado = child.wait().await;

            let mut status = job_bg.status.lock().await;
            if *status == JobStatus::Cancelled {
                // render_cancel já marcou o status; não sobrescreve.
                return;
            }
            *status = match resultado {
                Ok(saida) if saida.success() => JobStatus::Done,
                Ok(saida) => interpretar_falha(saida, &stderr_completo),
                Err(erro) => JobStatus::Failed(format!("falha ao esperar o processo: {erro}")),
            };
        });

        Ok(job)
    }

    pub async fn get(&self, job_id: &str) -> Option<Arc<RenderJob>> {
        self.jobs.lock().await.get(job_id).cloned()
    }

    /// Encerra o processo filho do job (sinal `SIGTERM` de verdade — ao
    /// contrário do `render_queue_stop` do original).
    ///
    /// A checagem de status e a troca para `Cancelled` acontecem sob o
    /// **mesmo** lock, sem soltá-lo entre as duas: isso evita uma corrida
    /// com a tarefa de fundo que lê o resultado do processo (ver `spawn`),
    /// que faz a mesma checagem (`if *status == Cancelled { return }`)
    /// antes de gravar `Done`/`Failed`. Sem isso, um job que termina com
    /// sucesso bem na hora de um `render_cancel` poderia ser reportado
    /// incorretamente como cancelado.
    pub async fn cancel(&self, job_id: &str) -> Result<(), EngineError> {
        let job = self
            .get(job_id)
            .await
            .ok_or_else(|| EngineError::InvalidText(format!("job não encontrado: {job_id}")))?;

        let mut status = job.status.lock().await;
        match *status {
            JobStatus::Queued => {
                // Ainda nem criou o processo filho: só marca cancelado. A
                // tarefa de fundo, ao finalmente pegar a vez no
                // render_lock, vê isso e nunca chega a spawnar nada.
                *status = JobStatus::Cancelled;
                Ok(())
            }
            JobStatus::Running => {
                let pid = job.pid.load(Ordering::Relaxed);
                if pid == 0 {
                    return Err(EngineError::InvalidText(
                        "este job está marcado como em andamento mas ainda não tem um processo \
                         associado; tente de novo em um instante"
                            .to_string(),
                    ));
                }
                kill_pid(pid)?;
                *status = JobStatus::Cancelled;
                Ok(())
            }
            _ => Err(EngineError::InvalidText(format!(
                "job \"{job_id}\" já não está em andamento (status: {})",
                status.as_str()
            ))),
        }
    }
}

fn interpretar_falha(status: std::process::ExitStatus, stderr: &str) -> JobStatus {
    #[cfg(unix)]
    {
        use std::os::unix::process::ExitStatusExt;
        if let Some(sinal) = status.signal() {
            return JobStatus::Failed(format!("o motor de render travou (sinal {sinal})"));
        }
    }
    let codigo = status.code().unwrap_or(-1);
    JobStatus::Failed(format!("código {codigo}: {}", stderr.trim()))
}

/// Lê a porcentagem de uma linha de progresso do `melt`/libmlt, no formato
/// `"Current Frame: N, percentage: P"` (confirmado rodando `melt` neste
/// ambiente — ver docs/06-testes.md).
fn parse_percentage(line: &str) -> Option<u8> {
    let idx = line.rfind("percentage:")?;
    let rest = &line[idx + "percentage:".len()..];
    let digits: String = rest
        .chars()
        .skip_while(|c| c.is_whitespace())
        .take_while(|c| c.is_ascii_digit())
        .collect();
    digits.parse::<u8>().ok()
}

/// Envia `SIGTERM` para um processo pelo pid.
fn kill_pid(pid: u32) -> Result<(), EngineError> {
    // SAFETY: `pid` vem de `tokio::process::Child::id()` de um processo
    // que este próprio servidor spawnou (nunca de entrada externa).
    // `libc::kill` com um pid que já terminou devolve -1/ESRCH, tratado
    // abaixo como sucesso (o job já não está mais rodando de qualquer jeito).
    let resultado = unsafe { libc::kill(pid as libc::pid_t, libc::SIGTERM) };
    if resultado != 0 {
        let erro = std::io::Error::last_os_error();
        if erro.raw_os_error() != Some(libc::ESRCH) {
            return Err(EngineError::InvalidText(format!(
                "falha ao encerrar o processo {pid}: {erro}"
            )));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn parse_percentage_le_o_formato_do_melt() {
        assert_eq!(
            super::parse_percentage("Current Frame:          6, percentage:         12"),
            Some(12)
        );
        assert_eq!(super::parse_percentage("nada a ver aqui"), None);
        assert_eq!(super::parse_percentage("percentage:"), None);
    }
}
