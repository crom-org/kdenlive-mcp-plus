//! `subtitle_import_srt`, `subtitle_export_srt`.
//!
//! Formato `.srt` em si é público e bem documentado (ver `model::srt`);
//! isto cria/lê clipes de título normais (produtor `pango`, ver
//! `tools::title`) numa trilha — não há nenhuma trilha "de legenda"
//! especial do Kdenlive sendo emulada aqui.

use std::path::PathBuf;

use serde_json::{Value, json};

use crate::mcp::params::{SubtitleExportSrtParams, SubtitleImportSrtParams};
use crate::mcp::server::{erro, KdenliveMcpServer};
use crate::model::document::{self, TrackKind};
use crate::model::srt::{self, SrtEntry};

impl KdenliveMcpServer {
    pub(crate) async fn subtitle_import_srt_impl(&self, params: SubtitleImportSrtParams) -> Value {
        let caminho = PathBuf::from(&params.path);
        let texto = match std::fs::read_to_string(&caminho) {
            Ok(t) => t,
            Err(e) => return erro(format!("falha ao ler {}: {e}", caminho.display())),
        };

        let descricao = format!("importar legendas de \"{}\"", caminho.display());
        let resultado = self
            .with_session_mut(|session| {
                let fps = document::profile_fps(&session.doc);
                let entradas = srt::parse(&texto, fps)
                    .map_err(|e| crate::model::ModelError::Invalid(format!("arquivo SRT inválido: {e}")))?;

                session.snapshot_before(descricao);

                let track_id = match &params.track_id {
                    Some(id) => id.clone(),
                    None => {
                        let novo_id = session.alloc_playlist_id();
                        document::add_track(&mut session.doc, &novo_id, TrackKind::Video)?;
                        novo_id
                    }
                };

                let mut clip_refs = Vec::with_capacity(entradas.len());
                for entrada in &entradas {
                    let duracao = (entrada.end_frame - entrada.start_frame).max(1);
                    let producer_id = session.alloc_producer_id();
                    document::add_title_clip(
                        &mut session.doc,
                        &producer_id,
                        &entrada.text,
                        duracao,
                        &params.font_family,
                        params.font_size,
                        &params.color,
                    )?;
                    let indice = document::add_clip_to_track(
                        &mut session.doc,
                        &track_id,
                        &producer_id,
                        0,
                        duracao - 1,
                        entrada.start_frame,
                    )?;
                    clip_refs.push(format!("{track_id}:{indice}"));
                }

                session.mark_modified();
                Ok((track_id, clip_refs))
            })
            .await;

        match resultado {
            Ok((track_id, clip_refs)) => json!({
                "ok": true,
                "track_id": track_id,
                "quantidade_legendas": clip_refs.len(),
                "clip_refs": clip_refs,
            }),
            Err(valor_erro) => valor_erro,
        }
    }

    pub(crate) async fn subtitle_export_srt_impl(&self, params: SubtitleExportSrtParams) -> Value {
        let resultado = self
            .with_session(|session| {
                let fps = document::profile_fps(&session.doc);
                let entradas: Vec<SrtEntry> = document::list_clips_with_position(&session.doc, &params.track_id)
                    .into_iter()
                    .filter_map(|(_, producer_id, in_f, out_f, position)| {
                        let producer = document::find_producer(&session.doc, &producer_id)?;
                        if document::get_property(producer, "mlt_service").as_deref() != Some("pango") {
                            return None;
                        }
                        let text = document::get_property(producer, "text")?;
                        Some(SrtEntry {
                            start_frame: position,
                            end_frame: position + (out_f - in_f + 1),
                            text,
                        })
                    })
                    .collect();
                (entradas, fps)
            })
            .await;

        let (entradas, fps) = match resultado {
            Ok(v) => v,
            Err(valor_erro) => return valor_erro,
        };

        if entradas.is_empty() {
            return erro(format!(
                "nenhum clipe de título (texto) encontrado na trilha \"{}\"",
                params.track_id
            ));
        }

        let texto = srt::write(&entradas, fps);
        let caminho = PathBuf::from(&params.path);
        if let Err(e) = std::fs::write(&caminho, texto) {
            return erro(format!("falha ao escrever {}: {e}", caminho.display()));
        }

        json!({ "ok": true, "caminho": caminho.display().to_string(), "quantidade_legendas": entradas.len() })
    }
}
