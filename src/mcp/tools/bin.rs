//! `bin_import_clip`, `bin_list_clips`, `bin_get_clip_details`,
//! `bin_remove_clip`.

use std::path::{Path, PathBuf};

use serde_json::{Value, json};

use crate::mcp::ffprobe;
use crate::mcp::params::{BinClipIdParams, BinImportClipParams};
use crate::mcp::server::{KdenliveMcpServer, erro};
use crate::model::document;

fn clip_json(doc: &xmltree::Element, id: &str) -> Option<Value> {
    let producer = document::find_producer(doc, id)?;
    let recurso = document::get_property(producer, "resource");
    let existe = recurso.as_deref().map(|r| Path::new(r).exists());
    Some(json!({
        "clip_id": id,
        "nome": document::get_property(producer, "kdenlive:clipname"),
        "resource": recurso,
        "in_frame": producer.attributes.get("in").and_then(|v| v.parse::<i64>().ok()).unwrap_or(0),
        "out_frame": producer.attributes.get("out").and_then(|v| v.parse::<i64>().ok()).unwrap_or(0),
        "arquivo_existe": existe,
    }))
}

impl KdenliveMcpServer {
    pub(crate) async fn bin_import_clip_impl(&self, params: BinImportClipParams) -> Value {
        let caminho = PathBuf::from(&params.path);
        if !caminho.exists() {
            return erro(format!("arquivo de mídia não encontrado: {}", caminho.display()));
        }

        let fps = match self.with_session(|s| document::profile_fps(&s.doc)).await {
            Ok(fps) => fps,
            Err(valor_erro) => return valor_erro,
        };

        let quadros = ffprobe::probe_duration_frames(&caminho, fps).await;
        let (in_frame, out_frame) = match quadros {
            Some(n) if n > 0 => (0, n - 1),
            _ => (0, 0),
        };
        let nota = quadros.is_none().then(|| {
            "duração não sondada (ffprobe indisponível ou falhou); in/out ficaram em 0"
                .to_string()
        });

        let nome = params.name.clone().unwrap_or_else(|| {
            caminho
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_else(|| caminho.display().to_string())
        });
        let resource = caminho.display().to_string();
        let descricao = format!("importar clipe \"{nome}\"");

        let resultado = self
            .with_session_mut(|session| {
                session.snapshot_before(descricao);
                let id = session.alloc_producer_id();
                document::add_bin_clip(&mut session.doc, &id, &resource, &nome, in_frame, out_frame)?;
                session.mark_modified();
                Ok(id)
            })
            .await;

        match resultado {
            Ok(id) => json!({
                "ok": true,
                "clip_id": id,
                "in_frame": in_frame,
                "out_frame": out_frame,
                "nota": nota,
            }),
            Err(valor_erro) => valor_erro,
        }
    }

    pub(crate) async fn bin_list_clips_impl(&self) -> Value {
        let resultado = self
            .with_session(|session| {
                document::bin_clip_ids(&session.doc)
                    .into_iter()
                    .filter_map(|id| clip_json(&session.doc, &id))
                    .collect::<Vec<_>>()
            })
            .await;
        match resultado {
            Ok(lista) => json!({ "ok": true, "clipes": lista }),
            Err(valor_erro) => valor_erro,
        }
    }

    pub(crate) async fn bin_get_clip_details_impl(&self, params: BinClipIdParams) -> Value {
        let resultado = self
            .with_session(|session| clip_json(&session.doc, &params.clip_id))
            .await;
        match resultado {
            Ok(Some(mut detalhes)) => {
                detalhes["ok"] = json!(true);
                detalhes
            }
            Ok(None) => erro(format!("clipe não encontrado: {}", params.clip_id)),
            Err(valor_erro) => valor_erro,
        }
    }

    pub(crate) async fn bin_remove_clip_impl(&self, params: BinClipIdParams) -> Value {
        let descricao = format!("remover clipe \"{}\" da bin", params.clip_id);
        let resultado = self
            .with_session_mut(|session| {
                session.snapshot_before(descricao);
                document::remove_bin_clip(&mut session.doc, &params.clip_id)?;
                session.mark_modified();
                Ok(())
            })
            .await;
        match resultado {
            Ok(()) => json!({ "ok": true, "clip_id": params.clip_id }),
            Err(valor_erro) => valor_erro,
        }
    }
}
