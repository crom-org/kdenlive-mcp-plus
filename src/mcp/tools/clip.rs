//! `clip_get_properties`, `clip_set_speed`, `clip_reverse`,
//! `clip_set_opacity`, `audio_set_levels`, `audio_normalize`,
//! `clip_set_transform` (ferramentas de **clipe da timeline**, por
//! `clip_ref` — diferente de `clip_set_color`/`clip_add_note`, que são de
//! bin e ficam em `tools/bin.rs`).

use serde_json::{Value, json};

use crate::mcp::params::{
    AudioNormalizeParams, AudioSetLevelsParams, ClipRefParams, ClipSetOpacityParams, ClipSetSpeedParams,
    ClipSetTransformParams,
};
use crate::mcp::server::{KdenliveMcpServer, erro, parse_clip_ref};
use crate::model::document;

impl KdenliveMcpServer {
    pub(crate) async fn clip_get_properties_impl(&self, params: ClipRefParams) -> Value {
        let (track_id, indice) = match parse_clip_ref(&params.clip_ref) {
            Ok(v) => v,
            Err(e) => return e,
        };
        let resultado = self
            .with_session(|session| {
                let entry = document::get_clip(&session.doc, &track_id, indice)?;
                let producer_id = entry.attributes.get("producer")?.clone();
                Some(json!({
                    "ok": true,
                    "clip_ref": params.clip_ref,
                    "producer_id": producer_id,
                    "in_frame": entry.attributes.get("in").and_then(|v| v.parse::<i64>().ok()).unwrap_or(0),
                    "out_frame": entry.attributes.get("out").and_then(|v| v.parse::<i64>().ok()).unwrap_or(0),
                    "velocidade": document::clip_speed(&session.doc, &track_id, indice),
                    "invertido": document::clip_speed(&session.doc, &track_id, indice) < 0.0,
                    "filtros": document::list_filters(&session.doc, &track_id, indice)
                        .into_iter()
                        .map(|(id, servico)| json!({ "filter_id": id, "service": servico }))
                        .collect::<Vec<_>>(),
                }))
            })
            .await;
        match resultado {
            Ok(Some(info)) => info,
            Ok(None) => erro(format!("clipe não encontrado: {}", params.clip_ref)),
            Err(valor_erro) => valor_erro,
        }
    }

    pub(crate) async fn clip_set_speed_impl(&self, params: ClipSetSpeedParams) -> Value {
        let (track_id, indice) = match parse_clip_ref(&params.clip_ref) {
            Ok(v) => v,
            Err(e) => return e,
        };
        let descricao = format!("definir velocidade do clipe {} para {}", params.clip_ref, params.speed);
        let resultado = self
            .with_session_mut(|session| {
                session.snapshot_before(descricao);
                document::set_clip_speed(&mut session.doc, &track_id, indice, params.speed)?;
                session.mark_modified();
                Ok(())
            })
            .await;
        match resultado {
            Ok(()) => json!({ "ok": true, "clip_ref": params.clip_ref, "velocidade": params.speed }),
            Err(valor_erro) => valor_erro,
        }
    }

    pub(crate) async fn clip_reverse_impl(&self, params: ClipRefParams) -> Value {
        let (track_id, indice) = match parse_clip_ref(&params.clip_ref) {
            Ok(v) => v,
            Err(e) => return e,
        };
        let descricao = format!("inverter clipe {}", params.clip_ref);
        let resultado = self
            .with_session_mut(|session| {
                session.snapshot_before(descricao);
                document::reverse_clip(&mut session.doc, &track_id, indice)?;
                session.mark_modified();
                Ok(document::clip_speed(&session.doc, &track_id, indice))
            })
            .await;
        match resultado {
            Ok(velocidade) => json!({ "ok": true, "clip_ref": params.clip_ref, "velocidade": velocidade }),
            Err(valor_erro) => valor_erro,
        }
    }

    pub(crate) async fn clip_set_opacity_impl(&self, params: ClipSetOpacityParams) -> Value {
        if !(0.0..=1.0).contains(&params.opacity) {
            return erro("opacity precisa estar entre 0.0 e 1.0".to_string());
        }
        let (track_id, indice) = match parse_clip_ref(&params.clip_ref) {
            Ok(v) => v,
            Err(e) => return e,
        };
        let descricao = format!("definir opacidade do clipe {} para {}", params.clip_ref, params.opacity);
        let resultado = self
            .with_session_mut(|session| {
                session.snapshot_before(descricao);
                let filter_id = document::set_or_create_filter_param(
                    &mut session.doc,
                    &track_id,
                    indice,
                    "brightness",
                    "alpha",
                    &params.opacity.to_string(),
                )?;
                session.mark_modified();
                Ok(filter_id)
            })
            .await;
        match resultado {
            Ok(filter_id) => json!({
                "ok": true,
                "clip_ref": params.clip_ref,
                "opacity": params.opacity,
                "filter_id": filter_id,
            }),
            Err(valor_erro) => valor_erro,
        }
    }

    pub(crate) async fn audio_set_levels_impl(&self, params: AudioSetLevelsParams) -> Value {
        let (track_id, indice) = match parse_clip_ref(&params.clip_ref) {
            Ok(v) => v,
            Err(e) => return e,
        };
        let descricao = format!("definir nível de áudio do clipe {} para {} dB", params.clip_ref, params.db);
        let resultado = self
            .with_session_mut(|session| {
                session.snapshot_before(descricao);
                let filter_id = document::audio_set_level(&mut session.doc, &track_id, indice, params.db)?;
                session.mark_modified();
                Ok(filter_id)
            })
            .await;
        match resultado {
            Ok(filter_id) => json!({ "ok": true, "clip_ref": params.clip_ref, "db": params.db, "filter_id": filter_id }),
            Err(valor_erro) => valor_erro,
        }
    }

    pub(crate) async fn audio_normalize_impl(&self, params: AudioNormalizeParams) -> Value {
        let (track_id, indice) = match parse_clip_ref(&params.clip_ref) {
            Ok(v) => v,
            Err(e) => return e,
        };
        let descricao = format!("normalizar áudio do clipe {} para {} dB", params.clip_ref, params.target_db);
        let resultado = self
            .with_session_mut(|session| {
                session.snapshot_before(descricao);
                let filter_id = document::audio_normalize(&mut session.doc, &track_id, indice, params.target_db)?;
                session.mark_modified();
                Ok(filter_id)
            })
            .await;
        match resultado {
            Ok(filter_id) => json!({
                "ok": true,
                "clip_ref": params.clip_ref,
                "target_db": params.target_db,
                "filter_id": filter_id,
            }),
            Err(valor_erro) => valor_erro,
        }
    }

    pub(crate) async fn clip_set_transform_impl(&self, params: ClipSetTransformParams) -> Value {
        let (track_id, indice) = match parse_clip_ref(&params.clip_ref) {
            Ok(v) => v,
            Err(e) => return e,
        };
        let descricao = format!("definir transformação do clipe {}", params.clip_ref);
        let resultado = self
            .with_session_mut(|session| {
                session.snapshot_before(descricao);
                let filter_id = document::set_clip_transform(
                    &mut session.doc,
                    &track_id,
                    indice,
                    params.x,
                    params.y,
                    params.width,
                    params.height,
                    params.rotation,
                )?;
                session.mark_modified();
                Ok(filter_id)
            })
            .await;
        match resultado {
            Ok(filter_id) => json!({ "ok": true, "clip_ref": params.clip_ref, "filter_id": filter_id }),
            Err(valor_erro) => valor_erro,
        }
    }
}
