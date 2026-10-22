//! `render_queue_add`, `render_queue_list`, `render_queue_status`,
//! `render_queue_start`, `render_queue_stop`.

use std::path::PathBuf;
use std::sync::atomic::Ordering;

use serde_json::{Value, json};

use crate::engine::preset;
use crate::engine::queue::{QueueEntry, QueueEntryStatus, QueueSpec};
use crate::mcp::params::RenderQueueAddParams;
use crate::mcp::server::{KdenliveMcpServer, erro};

async fn entry_json(entrada: &QueueEntry) -> Value {
    let status = entrada.status.lock().await;
    let mut valor = json!({
        "queue_id": entrada.id,
        "output_path": entrada.spec.output_path.display().to_string(),
        "preset": entrada.spec.preset,
        "status": status.as_str(),
        "percentual": entrada.percent.load(Ordering::Relaxed),
    });
    if let QueueEntryStatus::Failed(detalhe) = &*status {
        valor["detalhe_erro"] = json!(detalhe);
    }
    valor
}

impl KdenliveMcpServer {
    pub(crate) async fn render_queue_add_impl(&self, params: RenderQueueAddParams) -> Value {
        let output_path = PathBuf::from(&params.output_path);

        let opts = match preset::resolve_preset(&params.preset, params.overwrite) {
            Ok(opts) => opts,
            Err(e) => return erro(e.to_string()),
        };
        match output_path.extension().and_then(|e| e.to_str()) {
            Some(ext) if ext.eq_ignore_ascii_case(opts.extension) => {}
            Some(ext) => {
                return erro(format!(
                    "a extensão de output_path (\".{ext}\") não combina com o preset \"{}\", \
                     que espera \".{}\"",
                    params.preset, opts.extension
                ));
            }
            None => {
                return erro(format!(
                    "output_path precisa terminar em \".{}\" para o preset \"{}\"",
                    opts.extension, params.preset
                ));
            }
        }

        let project_path = match self
            .resolve_project_path_for_render(params.project_path.as_deref())
            .await
        {
            Ok(p) => p,
            Err(e) => return e,
        };
        if !project_path.exists() {
            return erro(format!("projeto não encontrado: {}", project_path.display()));
        }
        if output_path.exists() && !params.overwrite {
            return erro(format!(
                "o arquivo de saída já existe: {} (chame novamente com overwrite=true)",
                output_path.display()
            ));
        }

        let entrada = self
            .queue
            .add(QueueSpec {
                project_path,
                output_path,
                preset: params.preset,
                overwrite: params.overwrite,
            })
            .await;
        let valor = entry_json(&entrada).await;
        json!({ "ok": true, "queue_id": entrada.id, "item": valor })
    }

    pub(crate) async fn render_queue_list_impl(&self) -> Value {
        let entradas = self.queue.list().await;
        let mut itens = Vec::with_capacity(entradas.len());
        for e in &entradas {
            itens.push(entry_json(e).await);
        }
        json!({ "ok": true, "fila": itens })
    }

    pub(crate) async fn render_queue_status_impl(&self) -> Value {
        let (pendente, rodando, concluido, falhou, cancelado) = self.queue.status_summary().await;
        json!({
            "ok": true,
            "processando": self.queue.is_processing(),
            "pendente": pendente,
            "em_andamento": rodando,
            "concluido": concluido,
            "falhou": falhou,
            "cancelado": cancelado,
        })
    }

    pub(crate) async fn render_queue_start_impl(&self) -> Value {
        match self.queue.start(self.render_lock.clone(), self.jobs.clone()).await {
            Ok(()) => json!({ "ok": true, "processando": true }),
            Err(e) => erro(e.to_string()),
        }
    }

    pub(crate) async fn render_queue_stop_impl(&self) -> Value {
        match self.queue.stop() {
            Ok(()) => json!({ "ok": true, "processando": false }),
            Err(e) => erro(e.to_string()),
        }
    }
}
