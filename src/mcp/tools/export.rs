//! `export_xml`, `export_render`, `render_progress`, `render_cancel`,
//! `preview_frame`, `export_thumbnail`.

use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use serde_json::{Value, json};

use crate::engine::jobs::JobStatus;
use crate::engine::{preset, worker};
use crate::mcp::ffprobe;
use crate::mcp::params::{ExportRenderParams, ExportThumbnailParams, ExportXmlParams, JobIdParams, PreviewFrameParams};
use crate::mcp::server::{KdenliveMcpServer, erro};
use crate::model::document;

static CONTADOR_TEMP: AtomicU64 = AtomicU64::new(0);

impl KdenliveMcpServer {
    /// Resolve o caminho do projeto a usar num render/preview: o `given`
    /// explícito, se houver, ou um arquivo temporário com o XML da sessão
    /// aberta. Este arquivo temporário **não é removido** nesta fase (ver
    /// docs/09-riscos-e-bugs.md): seu ciclo de vida precisa sobreviver a um
    /// render assíncrono (`wait=false`), cujo fim este método não espera.
    pub(crate) async fn resolve_project_path_for_render(&self, given: Option<&str>) -> Result<PathBuf, Value> {
        if let Some(caminho) = given {
            return Ok(PathBuf::from(caminho));
        }

        let xml = self
            .with_session(|session| document::to_string_pretty(&session.doc))
            .await?
            .map_err(|e| erro(e.to_string()))?;

        let sufixo = CONTADOR_TEMP.fetch_add(1, Ordering::Relaxed);
        let caminho = std::env::temp_dir().join(format!(
            "kdenlive_mcp_plus_sessao_{}_{sufixo}.xml",
            std::process::id()
        ));
        std::fs::write(&caminho, xml)
            .map_err(|e| erro(format!("falha ao escrever o projeto temporário da sessão: {e}")))?;
        Ok(caminho)
    }

    pub(crate) async fn export_xml_impl(&self, params: ExportXmlParams) -> Value {
        let xml = match self.with_session(|s| document::to_string_pretty(&s.doc)).await {
            Ok(Ok(xml)) => xml,
            Ok(Err(e)) => return erro(e.to_string()),
            Err(valor_erro) => return valor_erro,
        };

        match params.path {
            Some(caminho_texto) => {
                let caminho = PathBuf::from(&caminho_texto);
                if let Err(e) = std::fs::write(&caminho, &xml) {
                    return erro(format!("falha ao escrever {}: {e}", caminho.display()));
                }
                json!({ "ok": true, "caminho": caminho.display().to_string() })
            }
            None => json!({ "ok": true, "xml": xml }),
        }
    }

    pub(crate) async fn export_render_impl(&self, params: ExportRenderParams) -> Value {
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

        let intervalo = match (params.start_frame, params.end_frame) {
            (Some(a), Some(b)) if a >= 0 && b >= a => Some((a, b)),
            (Some(_), Some(_)) => return erro("intervalo inválido: start_frame não pode ser negativo nem maior que end_frame"),
            (None, None) => None,
            _ => return erro("informe start_frame e end_frame juntos (ou nenhum dos dois, para renderizar tudo)"),
        };
        if intervalo.is_some() && !params.wait {
            return erro("renderizar só um trecho (start_frame/end_frame) funciona apenas com wait=true");
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

        if params.wait {
            let _guard = self.render_lock.lock().await;
            if let Err(e) =
                worker::render_range_isolated(&project_path, &output_path, &params.preset, params.overwrite, intervalo)
                    .await
            {
                return erro(e.to_string());
            }
            let tamanho_bytes = std::fs::metadata(&output_path).ok().map(|m| m.len());
            let (duracao_segundos, notas) = ffprobe::probe_duration_seconds(&output_path).await;
            json!({
                "ok": true,
                "caminho_saida": output_path.display().to_string(),
                "tamanho_bytes": tamanho_bytes,
                "preset_usado": params.preset,
                "duracao_segundos": duracao_segundos,
                "notas": notas,
            })
        } else {
            match self
                .jobs
                .spawn(
                    self.render_lock.clone(),
                    &project_path,
                    &output_path,
                    &params.preset,
                    params.overwrite,
                )
                .await
            {
                Ok(job) => json!({ "ok": true, "job_id": job.id, "status": job.status().await.as_str() }),
                Err(e) => erro(e.to_string()),
            }
        }
    }

    pub(crate) async fn render_progress_impl(&self, params: JobIdParams) -> Value {
        let Some(job) = self.jobs.get(&params.job_id).await else {
            return erro(format!("job não encontrado: {}", params.job_id));
        };
        let status = job.status().await;
        let mut resposta = json!({
            "ok": true,
            "job_id": params.job_id,
            "status": status.as_str(),
            "percentual": job.percent(),
        });
        match &status {
            JobStatus::Failed(detalhe) => resposta["detalhe_erro"] = json!(detalhe),
            JobStatus::Done => resposta["caminho_saida"] = json!(job.output_path.display().to_string()),
            _ => {}
        }
        resposta
    }

    pub(crate) async fn render_cancel_impl(&self, params: JobIdParams) -> Value {
        match self.jobs.cancel(&params.job_id).await {
            Ok(()) => json!({ "ok": true, "job_id": params.job_id, "status": "cancelado" }),
            Err(e) => erro(e.to_string()),
        }
    }

    pub(crate) async fn preview_frame_impl(&self, params: PreviewFrameParams) -> Value {
        let output_path = PathBuf::from(&params.output_path);
        let extensao_ok = output_path
            .extension()
            .and_then(|e| e.to_str())
            .is_some_and(|e| e.eq_ignore_ascii_case("png"));
        if !extensao_ok {
            return erro("output_path precisa terminar em \".png\"".to_string());
        }
        if params.frame < 0 {
            return erro("frame não pode ser negativo".to_string());
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

        let _guard = self.render_lock.lock().await;
        match worker::render_frame_isolated(&project_path, &output_path, params.frame, params.overwrite)
            .await
        {
            Ok(()) => json!({
                "ok": true,
                "caminho_saida": output_path.display().to_string(),
                "quadro": params.frame,
            }),
            Err(e) => erro(e.to_string()),
        }
    }

    /// Gera uma miniatura de um vídeo **já renderizado** (não um projeto
    /// MLT) com `ffmpeg` diretamente — um subprocesso comum, sem nenhuma
    /// chamada ao libmlt.
    pub(crate) async fn export_thumbnail_impl(&self, params: ExportThumbnailParams) -> Value {
        let video_path = PathBuf::from(&params.video_path);
        if !video_path.exists() {
            return erro(format!("vídeo não encontrado: {}", video_path.display()));
        }
        let output_path = PathBuf::from(&params.output_path);
        if output_path.exists() && !params.overwrite {
            return erro(format!(
                "o arquivo de saída já existe: {} (chame novamente com overwrite=true)",
                output_path.display()
            ));
        }
        if params.at_seconds < 0.0 {
            return erro("at_seconds não pode ser negativo".to_string());
        }

        let saida = tokio::process::Command::new("ffmpeg")
            .arg("-y")
            .arg("-ss")
            .arg(params.at_seconds.to_string())
            .arg("-i")
            .arg(&video_path)
            .arg("-frames:v")
            .arg("1")
            .arg(&output_path)
            .output()
            .await;

        match saida {
            Ok(saida) if saida.status.success() && output_path.exists() => json!({
                "ok": true,
                "caminho_saida": output_path.display().to_string(),
                "at_seconds": params.at_seconds,
            }),
            Ok(saida) => erro(format!(
                "ffmpeg terminou com erro ao gerar a miniatura: {}",
                String::from_utf8_lossy(&saida.stderr).trim()
            )),
            Err(e) => erro(format!("não consegui rodar o ffmpeg: {e}")),
        }
    }
}
