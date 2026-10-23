//! `track_get_info`, `track_set_name`, `track_set_locked`,
//! `track_set_hidden`, `track_set_muted`, `track_move_up`, `track_move_down`.

use serde_json::{Value, json};

use crate::mcp::params::{TrackBoolParams, TrackIdParams, TrackSetNameParams};
use crate::mcp::server::{KdenliveMcpServer, erro};
use crate::model::document::{self, TrackKind};

fn track_info_json(doc: &xmltree::Element, track_id: &str) -> Option<Value> {
    if document::track_kind(doc, track_id).is_none() && document::find_track_playlist(doc, track_id).is_none() {
        return None;
    }
    let (oculta, muda) = document::track_hidden_muted(doc, track_id).unwrap_or((false, false));
    let clipes = document::list_clips(doc, track_id);
    Some(json!({
        "track_id": track_id,
        "kind": match document::track_kind(doc, track_id) { Some(TrackKind::Audio) => "audio", _ => "video" },
        "nome": document::track_name(doc, track_id),
        "travada": document::track_is_locked(doc, track_id),
        "oculta": oculta,
        "muda": muda,
        "quantidade_clipes": clipes.len(),
        "comprimento_quadros": document::find_track_playlist(doc, track_id).map(document::track_length).unwrap_or(0),
    }))
}

impl KdenliveMcpServer {
    pub(crate) async fn track_get_info_impl(&self, params: TrackIdParams) -> Value {
        let resultado = self
            .with_session(|session| track_info_json(&session.doc, &params.track_id))
            .await;
        match resultado {
            Ok(Some(mut info)) => {
                info["ok"] = json!(true);
                info
            }
            Ok(None) => erro(format!("trilha não encontrada: {}", params.track_id)),
            Err(valor_erro) => valor_erro,
        }
    }

    pub(crate) async fn track_set_name_impl(&self, params: TrackSetNameParams) -> Value {
        let descricao = format!("renomear trilha \"{}\" para \"{}\"", params.track_id, params.name);
        let resultado = self
            .with_session_mut(|session| {
                session.snapshot_before(descricao);
                document::track_set_name(&mut session.doc, &params.track_id, &params.name)?;
                session.mark_modified();
                Ok(())
            })
            .await;
        match resultado {
            Ok(()) => json!({ "ok": true, "track_id": params.track_id, "nome": params.name }),
            Err(valor_erro) => valor_erro,
        }
    }

    pub(crate) async fn track_set_locked_impl(&self, params: TrackBoolParams) -> Value {
        let descricao = format!(
            "{} trilha \"{}\"",
            if params.value { "travar" } else { "destravar" },
            params.track_id
        );
        let resultado = self
            .with_session_mut(|session| {
                session.snapshot_before(descricao);
                document::track_set_locked(&mut session.doc, &params.track_id, params.value)?;
                session.mark_modified();
                Ok(())
            })
            .await;
        match resultado {
            Ok(()) => json!({ "ok": true, "track_id": params.track_id, "travada": params.value }),
            Err(valor_erro) => valor_erro,
        }
    }

    pub(crate) async fn track_set_hidden_impl(&self, params: TrackBoolParams) -> Value {
        let descricao = format!(
            "{} trilha \"{}\"",
            if params.value { "ocultar" } else { "mostrar" },
            params.track_id
        );
        let resultado = self
            .with_session_mut(|session| {
                session.snapshot_before(descricao);
                document::track_set_hidden(&mut session.doc, &params.track_id, params.value)?;
                session.mark_modified();
                Ok(())
            })
            .await;
        match resultado {
            Ok(()) => json!({ "ok": true, "track_id": params.track_id, "oculta": params.value }),
            Err(valor_erro) => valor_erro,
        }
    }

    pub(crate) async fn track_set_muted_impl(&self, params: TrackBoolParams) -> Value {
        let descricao = format!(
            "{} trilha \"{}\"",
            if params.value { "mutar" } else { "desmutar" },
            params.track_id
        );
        let resultado = self
            .with_session_mut(|session| {
                session.snapshot_before(descricao);
                document::track_set_muted(&mut session.doc, &params.track_id, params.value)?;
                session.mark_modified();
                Ok(())
            })
            .await;
        match resultado {
            Ok(()) => json!({ "ok": true, "track_id": params.track_id, "muda": params.value }),
            Err(valor_erro) => valor_erro,
        }
    }

    pub(crate) async fn track_move_up_impl(&self, params: TrackIdParams) -> Value {
        let descricao = format!("mover trilha \"{}\" para cima", params.track_id);
        let resultado = self
            .with_session_mut(|session| {
                session.snapshot_before(descricao);
                document::track_move_up(&mut session.doc, &params.track_id)?;
                session.mark_modified();
                Ok(())
            })
            .await;
        match resultado {
            Ok(()) => json!({ "ok": true, "track_id": params.track_id }),
            Err(valor_erro) => valor_erro,
        }
    }

    pub(crate) async fn track_move_down_impl(&self, params: TrackIdParams) -> Value {
        let descricao = format!("mover trilha \"{}\" para baixo", params.track_id);
        let resultado = self
            .with_session_mut(|session| {
                session.snapshot_before(descricao);
                document::track_move_down(&mut session.doc, &params.track_id)?;
                session.mark_modified();
                Ok(())
            })
            .await;
        match resultado {
            Ok(()) => json!({ "ok": true, "track_id": params.track_id }),
            Err(valor_erro) => valor_erro,
        }
    }
}
