//! `timeline_add_track`, `timeline_remove_track`, `timeline_add_clip`,
//! `timeline_list`, `timeline_get_info`, `timeline_assemble_ranges`.

use serde_json::{Value, json};

use crate::mcp::params::{
    ClipRefParams, TimelineAddClipParams, TimelineAddTrackParams, TimelineAssembleRangesParams,
    TimelineMoveClipParams, TimelineRemoveTrackParams, TimelineSeekParams, TimelineSetZoomParams,
    TimelineSplitClipParams, TimelineTrimClipParams,
};
use crate::mcp::server::{KdenliveMcpServer, erro, parse_clip_ref};
use crate::model::document::{self, TrackKind};

fn parse_kind(texto: &str) -> Result<TrackKind, Value> {
    match texto {
        "video" => Ok(TrackKind::Video),
        "audio" => Ok(TrackKind::Audio),
        outro => Err(erro(format!(
            "kind inválido: \"{outro}\". Use \"video\" ou \"audio\""
        ))),
    }
}

fn track_json(doc: &xmltree::Element, track_id: &str) -> Value {
    let kind = document::track_kind(doc, track_id);
    let clipes = document::list_clips(doc, track_id);
    let comprimento = document::find_track_playlist(doc, track_id)
        .map(document::track_length)
        .unwrap_or(0);
    json!({
        "track_id": track_id,
        "kind": match kind { Some(TrackKind::Video) => "video", Some(TrackKind::Audio) => "audio", None => "desconhecido" },
        "quantidade_clipes": clipes.len(),
        "comprimento_quadros": comprimento,
    })
}

fn clip_entry_json(entry: &(usize, String, i64, i64), track_id: &str) -> Value {
    let (indice, producer_id, in_frame, out_frame) = entry;
    json!({
        "clip_ref": format!("{track_id}:{indice}"),
        "producer_id": producer_id,
        "in_frame": in_frame,
        "out_frame": out_frame,
    })
}

impl KdenliveMcpServer {
    pub(crate) async fn timeline_add_track_impl(&self, params: TimelineAddTrackParams) -> Value {
        let kind = match parse_kind(&params.kind) {
            Ok(k) => k,
            Err(e) => return e,
        };
        let descricao = format!("adicionar trilha de {}", params.kind);
        let resultado = self
            .with_session_mut(|session| {
                session.snapshot_before(descricao);
                let id = session.alloc_playlist_id();
                document::add_track(&mut session.doc, &id, kind)?;
                session.mark_modified();
                Ok(id)
            })
            .await;
        match resultado {
            Ok(track_id) => json!({ "ok": true, "track_id": track_id, "kind": params.kind }),
            Err(valor_erro) => valor_erro,
        }
    }

    pub(crate) async fn timeline_remove_track_impl(&self, params: TimelineRemoveTrackParams) -> Value {
        let descricao = format!("remover trilha \"{}\"", params.track_id);
        let resultado = self
            .with_session_mut(|session| {
                session.snapshot_before(descricao);
                document::remove_track(&mut session.doc, &params.track_id)?;
                session.mark_modified();
                Ok(())
            })
            .await;
        match resultado {
            Ok(()) => json!({ "ok": true, "track_id": params.track_id }),
            Err(valor_erro) => valor_erro,
        }
    }

    pub(crate) async fn timeline_add_clip_impl(&self, params: TimelineAddClipParams) -> Value {
        let descricao = format!(
            "posicionar clipe \"{}\" na trilha \"{}\" em {}",
            params.producer_id, params.track_id, params.position
        );
        let resultado = self
            .with_session_mut(|session| {
                session.snapshot_before(descricao);
                let indice = document::add_clip_to_track(
                    &mut session.doc,
                    &params.track_id,
                    &params.producer_id,
                    params.in_frame,
                    params.out_frame,
                    params.position,
                )?;
                session.mark_modified();
                Ok(indice)
            })
            .await;
        match resultado {
            Ok(indice) => json!({
                "ok": true,
                "clip_ref": format!("{}:{}", params.track_id, indice),
            }),
            Err(valor_erro) => valor_erro,
        }
    }

    pub(crate) async fn timeline_list_impl(&self) -> Value {
        let resultado = self
            .with_session(|session| {
                document::track_ids(&session.doc)
                    .into_iter()
                    .map(|id| track_json(&session.doc, &id))
                    .collect::<Vec<_>>()
            })
            .await;
        match resultado {
            Ok(trilhas) => json!({ "ok": true, "trilhas": trilhas }),
            Err(valor_erro) => valor_erro,
        }
    }

    pub(crate) async fn timeline_get_info_impl(&self) -> Value {
        let resultado = self
            .with_session(|session| {
                let doc = &session.doc;
                let trilhas: Vec<Value> = document::track_ids(doc)
                    .into_iter()
                    .map(|id| {
                        let mut t = track_json(doc, &id);
                        let clipes: Vec<Value> = document::list_clips(doc, &id)
                            .iter()
                            .map(|c| clip_entry_json(c, &id))
                            .collect();
                        t["clipes"] = json!(clipes);
                        t
                    })
                    .collect();
                json!({
                    "ok": true,
                    "trilhas": trilhas,
                    "duracao_quadros": document::project_duration(doc),
                })
            })
            .await;
        match resultado {
            Ok(info) => info,
            Err(valor_erro) => valor_erro,
        }
    }

    pub(crate) async fn timeline_assemble_ranges_impl(&self, params: TimelineAssembleRangesParams) -> Value {
        if params.ranges.is_empty() {
            return erro("ranges não pode ser vazio".to_string());
        }
        let descricao = format!(
            "montar {} intervalo(s) do produtor \"{}\" na trilha \"{}\"",
            params.ranges.len(),
            params.producer_id,
            params.track_id
        );
        let resultado = self
            .with_session_mut(|session| {
                session.snapshot_before(descricao);
                let mut posicao = params.position;
                let mut refs = Vec::with_capacity(params.ranges.len());
                for [entrada, saida] in &params.ranges {
                    let indice = document::add_clip_to_track(
                        &mut session.doc,
                        &params.track_id,
                        &params.producer_id,
                        *entrada,
                        *saida,
                        posicao,
                    )?;
                    refs.push(format!("{}:{}", params.track_id, indice));
                    posicao += saida - entrada + 1;
                }
                session.mark_modified();
                Ok(refs)
            })
            .await;
        match resultado {
            Ok(refs) => json!({ "ok": true, "clip_refs": refs }),
            Err(valor_erro) => valor_erro,
        }
    }

    pub(crate) async fn timeline_remove_clip_impl(&self, params: ClipRefParams) -> Value {
        let (track_id, indice) = match parse_clip_ref(&params.clip_ref) {
            Ok(v) => v,
            Err(e) => return e,
        };
        let descricao = format!("remover clipe {}", params.clip_ref);
        let resultado = self
            .with_session_mut(|session| {
                session.snapshot_before(descricao);
                document::remove_clip_from_track(&mut session.doc, &track_id, indice)?;
                session.mark_modified();
                Ok(())
            })
            .await;
        match resultado {
            Ok(()) => json!({ "ok": true, "clip_ref": params.clip_ref }),
            Err(valor_erro) => valor_erro,
        }
    }

    pub(crate) async fn timeline_move_clip_impl(&self, params: TimelineMoveClipParams) -> Value {
        let (track_id, indice) = match parse_clip_ref(&params.clip_ref) {
            Ok(v) => v,
            Err(e) => return e,
        };
        let descricao = format!("mover clipe {} para {}", params.clip_ref, params.new_position);
        let resultado = self
            .with_session_mut(|session| {
                session.snapshot_before(descricao);
                let novo_indice = document::move_clip(&mut session.doc, &track_id, indice, params.new_position)?;
                session.mark_modified();
                Ok(novo_indice)
            })
            .await;
        match resultado {
            Ok(novo_indice) => json!({ "ok": true, "clip_ref": format!("{track_id}:{novo_indice}") }),
            Err(valor_erro) => valor_erro,
        }
    }

    pub(crate) async fn timeline_trim_clip_impl(&self, params: TimelineTrimClipParams) -> Value {
        let (track_id, indice) = match parse_clip_ref(&params.clip_ref) {
            Ok(v) => v,
            Err(e) => return e,
        };
        let descricao = format!("aparar clipe {}", params.clip_ref);
        let resultado = self
            .with_session_mut(|session| {
                session.snapshot_before(descricao);
                document::trim_clip(&mut session.doc, &track_id, indice, params.in_frame, params.out_frame)?;
                session.mark_modified();
                Ok(())
            })
            .await;
        match resultado {
            Ok(()) => json!({
                "ok": true,
                "clip_ref": params.clip_ref,
                "in_frame": params.in_frame,
                "out_frame": params.out_frame,
            }),
            Err(valor_erro) => valor_erro,
        }
    }

    pub(crate) async fn timeline_split_clip_impl(&self, params: TimelineSplitClipParams) -> Value {
        let (track_id, indice) = match parse_clip_ref(&params.clip_ref) {
            Ok(v) => v,
            Err(e) => return e,
        };
        let descricao = format!("dividir clipe {} em {}", params.clip_ref, params.split_at);
        let resultado = self
            .with_session_mut(|session| {
                session.snapshot_before(descricao);
                let (primeiro, segundo) = document::split_clip(&mut session.doc, &track_id, indice, params.split_at)?;
                session.mark_modified();
                Ok((primeiro, segundo))
            })
            .await;
        match resultado {
            Ok((primeiro, segundo)) => json!({
                "ok": true,
                "clip_ref_primeiro": format!("{track_id}:{primeiro}"),
                "clip_ref_segundo": format!("{track_id}:{segundo}"),
            }),
            Err(valor_erro) => valor_erro,
        }
    }

    pub(crate) async fn timeline_get_duration_impl(&self) -> Value {
        match self.with_session(|s| document::project_duration(&s.doc)).await {
            Ok(duracao) => json!({ "ok": true, "duracao_quadros": duracao }),
            Err(valor_erro) => valor_erro,
        }
    }

    pub(crate) async fn timeline_get_position_impl(&self) -> Value {
        match self.with_session(|s| s.playhead).await {
            Ok(posicao) => json!({ "ok": true, "position": posicao }),
            Err(valor_erro) => valor_erro,
        }
    }

    pub(crate) async fn timeline_seek_impl(&self, params: TimelineSeekParams) -> Value {
        if params.position < 0 {
            return erro("position não pode ser negativa".to_string());
        }
        let mut guard = self.session.lock().await;
        match guard.as_mut() {
            Some(session) => {
                session.playhead = params.position;
                json!({ "ok": true, "position": params.position })
            }
            None => erro("nenhum projeto aberto; chame project_new ou project_open primeiro"),
        }
    }

    pub(crate) async fn timeline_get_zoom_impl(&self) -> Value {
        match self.with_session(|s| s.zoom).await {
            Ok(zoom) => json!({ "ok": true, "zoom": zoom }),
            Err(valor_erro) => valor_erro,
        }
    }

    pub(crate) async fn timeline_set_zoom_impl(&self, params: TimelineSetZoomParams) -> Value {
        if !(0.1..=10.0).contains(&params.zoom) {
            return erro("zoom precisa estar entre 0.1 e 10.0".to_string());
        }
        let mut guard = self.session.lock().await;
        match guard.as_mut() {
            Some(session) => {
                session.zoom = params.zoom;
                json!({ "ok": true, "zoom": params.zoom })
            }
            None => erro("nenhum projeto aberto; chame project_new ou project_open primeiro"),
        }
    }
}
