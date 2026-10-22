//! `keyframe_set`, `keyframe_list`, `keyframe_remove`.

use serde_json::{Value, json};

use crate::mcp::params::{KeyframeListParams, KeyframeRemoveParams, KeyframeSetParams};
use crate::mcp::server::{KdenliveMcpServer, parse_clip_ref};
use crate::model::document;

impl KdenliveMcpServer {
    pub(crate) async fn keyframe_set_impl(&self, params: KeyframeSetParams) -> Value {
        let (track_id, indice) = match parse_clip_ref(&params.clip_ref) {
            Ok(v) => v,
            Err(e) => return e,
        };
        let descricao = format!(
            "definir keyframe {}={} no quadro {} do filtro \"{}\" do clipe {}",
            params.param, params.value, params.frame, params.filter_id, params.clip_ref
        );
        let resultado = self
            .with_session_mut(|session| {
                session.snapshot_before(descricao);
                document::keyframe_set(
                    &mut session.doc,
                    &track_id,
                    indice,
                    &params.filter_id,
                    &params.param,
                    params.frame,
                    &params.value,
                )?;
                session.mark_modified();
                Ok(())
            })
            .await;
        match resultado {
            Ok(()) => json!({
                "ok": true,
                "clip_ref": params.clip_ref,
                "filter_id": params.filter_id,
                "param": params.param,
                "frame": params.frame,
                "value": params.value,
            }),
            Err(valor_erro) => valor_erro,
        }
    }

    pub(crate) async fn keyframe_list_impl(&self, params: KeyframeListParams) -> Value {
        let (track_id, indice) = match parse_clip_ref(&params.clip_ref) {
            Ok(v) => v,
            Err(e) => return e,
        };
        let resultado = self
            .with_session(|session| document::keyframe_list(&session.doc, &track_id, indice, &params.filter_id, &params.param))
            .await;
        match resultado {
            Ok(lista) => {
                let keyframes: Vec<Value> = lista
                    .into_iter()
                    .map(|(frame, value)| json!({ "frame": frame, "value": value }))
                    .collect();
                json!({ "ok": true, "clip_ref": params.clip_ref, "keyframes": keyframes })
            }
            Err(valor_erro) => valor_erro,
        }
    }

    pub(crate) async fn keyframe_remove_impl(&self, params: KeyframeRemoveParams) -> Value {
        let (track_id, indice) = match parse_clip_ref(&params.clip_ref) {
            Ok(v) => v,
            Err(e) => return e,
        };
        let descricao = format!(
            "remover keyframe do quadro {} do filtro \"{}\" do clipe {}",
            params.frame, params.filter_id, params.clip_ref
        );
        let resultado = self
            .with_session_mut(|session| {
                session.snapshot_before(descricao);
                document::keyframe_remove(&mut session.doc, &track_id, indice, &params.filter_id, &params.param, params.frame)?;
                session.mark_modified();
                Ok(())
            })
            .await;
        match resultado {
            Ok(()) => json!({ "ok": true, "clip_ref": params.clip_ref, "frame": params.frame }),
            Err(valor_erro) => valor_erro,
        }
    }
}
