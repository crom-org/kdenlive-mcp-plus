//! `marker_add`, `marker_list`, `marker_remove`.
//!
//! Marcadores gerais da timeline (guias da categoria 1 no Kdenlive) e
//! marcadores de clipe (`kdenlive:markers` do clipe da bin) — separados de
//! `guide_*` (ver `model::document`).

use serde_json::{Value, json};

use crate::mcp::params::{MarkerAddParams, MarkerIdParams};
use crate::mcp::server::KdenliveMcpServer;
use crate::model::document::{self, Marker};

fn marker_json(m: &Marker) -> Value {
    json!({ "marker_id": m.id, "position": m.position, "comment": m.comment, "clip_id": m.clip_id })
}

impl KdenliveMcpServer {
    pub(crate) async fn marker_add_impl(&self, params: MarkerAddParams) -> Value {
        let descricao = format!("adicionar marcador em {}", params.position);
        let resultado = self
            .with_session_mut(|session| {
                session.snapshot_before(descricao);
                let id = document::add_marker(&mut session.doc, params.position, &params.comment, params.clip_ref.as_deref())?;
                session.mark_modified();
                Ok(id)
            })
            .await;
        match resultado {
            Ok(id) => json!({
                "ok": true,
                "marker_id": id,
                "position": params.position,
                "comment": params.comment,
                "clip_ref": params.clip_ref,
            }),
            Err(valor_erro) => valor_erro,
        }
    }

    pub(crate) async fn marker_list_impl(&self) -> Value {
        let resultado = self
            .with_session(|session| document::list_markers(&session.doc).iter().map(marker_json).collect::<Vec<_>>())
            .await;
        match resultado {
            Ok(marcadores) => json!({ "ok": true, "marcadores": marcadores }),
            Err(valor_erro) => valor_erro,
        }
    }

    pub(crate) async fn marker_remove_impl(&self, params: MarkerIdParams) -> Value {
        let descricao = format!("remover marcador \"{}\"", params.marker_id);
        let resultado = self
            .with_session_mut(|session| {
                session.snapshot_before(descricao);
                document::remove_marker(&mut session.doc, &params.marker_id)?;
                session.mark_modified();
                Ok(())
            })
            .await;
        match resultado {
            Ok(()) => json!({ "ok": true, "marker_id": params.marker_id }),
            Err(valor_erro) => valor_erro,
        }
    }
}
