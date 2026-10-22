//! Fila de render (`render_queue_*`): várias tarefas de render enfileiradas
//! e processadas em sequência, uma por vez, quando `render_queue_start` é
//! chamado. `render_queue_stop` encerra de verdade o render em andamento
//! (via `JobRegistry::cancel`, que manda `SIGTERM` no processo filho) —
//! diferente do `render_queue_stop` do projeto original, que só marcava um
//! estado interno sem interromper nada (ver docs/09-riscos-e-bugs.md).

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicU8, Ordering};
use std::sync::Arc;
use std::time::Duration;

use tokio::sync::Mutex;

use super::jobs::{JobRegistry, JobStatus};
use super::EngineError;

#[derive(Debug, Clone)]
pub struct QueueSpec {
    pub project_path: PathBuf,
    pub output_path: PathBuf,
    pub preset: String,
    pub overwrite: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum QueueEntryStatus {
    Pending,
    Running,
    Done,
    Failed(String),
    Cancelled,
}

impl QueueEntryStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            QueueEntryStatus::Pending => "pendente",
            QueueEntryStatus::Running => "em andamento",
            QueueEntryStatus::Done => "concluído",
            QueueEntryStatus::Failed(_) => "falhou",
            QueueEntryStatus::Cancelled => "cancelado",
        }
    }
}

pub struct QueueEntry {
    pub id: String,
    pub spec: QueueSpec,
    pub status: Mutex<QueueEntryStatus>,
    pub percent: AtomicU8,
}

#[derive(Clone, Default)]
pub struct RenderQueue {
    entries: Arc<Mutex<Vec<Arc<QueueEntry>>>>,
    next_id: Arc<AtomicU64>,
    processing: Arc<AtomicBool>,
    stop_requested: Arc<AtomicBool>,
}

impl RenderQueue {
    pub fn new() -> Self {
        Self::default()
    }

    pub async fn add(&self, spec: QueueSpec) -> Arc<QueueEntry> {
        let id = format!("fila{}", self.next_id.fetch_add(1, Ordering::Relaxed));
        let entrada = Arc::new(QueueEntry {
            id,
            spec,
            status: Mutex::new(QueueEntryStatus::Pending),
            percent: AtomicU8::new(0),
        });
        self.entries.lock().await.push(entrada.clone());
        entrada
    }

    pub async fn list(&self) -> Vec<Arc<QueueEntry>> {
        self.entries.lock().await.clone()
    }

    pub async fn status_summary(&self) -> (usize, usize, usize, usize, usize) {
        let entradas = self.entries.lock().await;
        let mut pendente = 0;
        let mut rodando = 0;
        let mut concluido = 0;
        let mut falhou = 0;
        let mut cancelado = 0;
        for e in entradas.iter() {
            match *e.status.lock().await {
                QueueEntryStatus::Pending => pendente += 1,
                QueueEntryStatus::Running => rodando += 1,
                QueueEntryStatus::Done => concluido += 1,
                QueueEntryStatus::Failed(_) => falhou += 1,
                QueueEntryStatus::Cancelled => cancelado += 1,
            }
        }
        (pendente, rodando, concluido, falhou, cancelado)
    }

    pub fn is_processing(&self) -> bool {
        self.processing.load(Ordering::SeqCst)
    }

    /// Começa a processar a fila em segundo plano, um render por vez
    /// (compartilhando `render_lock` com `export_render`/`JobRegistry`, a
    /// mesma garantia da decisão D-016). Erro se a fila já estiver sendo
    /// processada.
    pub async fn start(
        &self,
        render_lock: Arc<Mutex<()>>,
        jobs: JobRegistry,
    ) -> Result<(), EngineError> {
        if self.processing.swap(true, Ordering::SeqCst) {
            return Err(EngineError::InvalidText(
                "a fila de render já está sendo processada".to_string(),
            ));
        }
        self.stop_requested.store(false, Ordering::SeqCst);

        let entradas = self.entries.lock().await.clone();
        let processing = self.processing.clone();
        let stop_requested = self.stop_requested.clone();

        tokio::spawn(async move {
            for entrada in entradas {
                {
                    let mut status = entrada.status.lock().await;
                    if stop_requested.load(Ordering::SeqCst) {
                        if *status == QueueEntryStatus::Pending {
                            *status = QueueEntryStatus::Cancelled;
                        }
                        continue;
                    }
                    if *status != QueueEntryStatus::Pending {
                        continue;
                    }
                    *status = QueueEntryStatus::Running;
                }

                let job = match jobs
                    .spawn(
                        render_lock.clone(),
                        &entrada.spec.project_path,
                        &entrada.spec.output_path,
                        &entrada.spec.preset,
                        entrada.spec.overwrite,
                    )
                    .await
                {
                    Ok(j) => j,
                    Err(e) => {
                        *entrada.status.lock().await = QueueEntryStatus::Failed(e.to_string());
                        continue;
                    }
                };

                loop {
                    entrada.percent.store(job.percent(), Ordering::Relaxed);
                    match job.status().await {
                        JobStatus::Done => {
                            *entrada.status.lock().await = QueueEntryStatus::Done;
                            break;
                        }
                        JobStatus::Failed(msg) => {
                            *entrada.status.lock().await = QueueEntryStatus::Failed(msg);
                            break;
                        }
                        JobStatus::Cancelled => {
                            *entrada.status.lock().await = QueueEntryStatus::Cancelled;
                            break;
                        }
                        JobStatus::Queued | JobStatus::Running => {
                            if stop_requested.load(Ordering::SeqCst) {
                                let _ = jobs.cancel(&job.id).await;
                            }
                            tokio::time::sleep(Duration::from_millis(150)).await;
                        }
                    }
                }
            }
            processing.store(false, Ordering::SeqCst);
        });

        Ok(())
    }

    /// Pede para a fila parar: o render em andamento é encerrado de
    /// verdade (processo filho morto), e as tarefas ainda pendentes ficam
    /// marcadas como canceladas, sem serem iniciadas.
    pub fn stop(&self) -> Result<(), EngineError> {
        if !self.processing.load(Ordering::SeqCst) {
            return Err(EngineError::InvalidText(
                "a fila de render não está sendo processada".to_string(),
            ));
        }
        self.stop_requested.store(true, Ordering::SeqCst);
        Ok(())
    }
}
