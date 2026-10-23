//! `clip_fade`, `clip_animate_transform`, `keyframe_set_many` (D-054/D-055):
//! fades e animações no formato que o próprio Kdenlive grava, editáveis na
//! interface dele.

use serde_json::{Value, json};

use crate::mcp::params::{ClipAnimateTransformParams, ClipFadeParams, KeyframeSetManyParams};
use crate::mcp::server::{KdenliveMcpServer, erro, parse_clip_ref};
use crate::model::document::{self, FadeKind, FadeMode, Interpolation, TransformKeyframe};

impl KdenliveMcpServer {
    pub(crate) async fn clip_fade_impl(&self, params: ClipFadeParams) -> Value {
        let (track_id, indice) = match parse_clip_ref(&params.clip_ref) {
            Ok(v) => v,
            Err(e) => return e,
        };
        let kind = match params.kind.as_str() {
            "in" => FadeKind::In,
            "out" => FadeKind::Out,
            outro => return erro(format!("kind inválido: \"{outro}\". Use \"in\" ou \"out\"")),
        };
        let mode = match params.mode.as_deref().unwrap_or("alpha") {
            "alpha" => FadeMode::Alpha,
            "black" => FadeMode::Black,
            outro => return erro(format!("mode inválido: \"{outro}\". Use \"alpha\" ou \"black\"")),
        };
        let descricao = format!("fade {} de {} quadros no clipe {}", params.kind, params.duration_frames, params.clip_ref);
        let resultado = self
            .with_session_mut(|session| {
                session.snapshot_before(descricao);
                let r = document::clip_fade(&mut session.doc, &track_id, indice, kind, params.duration_frames, mode)?;
                session.mark_modified();
                Ok(r)
            })
            .await;
        match resultado {
            Ok((filter_id, kdenlive_id)) => json!({
                "ok": true,
                "clip_ref": params.clip_ref,
                "filter_id": filter_id,
                "efeito_kdenlive": kdenlive_id,
            }),
            Err(valor_erro) => valor_erro,
        }
    }

    pub(crate) async fn clip_animate_transform_impl(&self, params: ClipAnimateTransformParams) -> Value {
        let (track_id, indice) = match parse_clip_ref(&params.clip_ref) {
            Ok(v) => v,
            Err(e) => return e,
        };
        let interp = match params.interpolation.as_deref().unwrap_or("linear") {
            "linear" => Interpolation::Linear,
            "smooth" => Interpolation::Smooth,
            "hold" => Interpolation::Hold,
            outro => {
                return erro(format!("interpolation inválida: \"{outro}\". Use \"linear\", \"smooth\" ou \"hold\""));
            }
        };
        let keyframes: Vec<TransformKeyframe> = params
            .keyframes
            .iter()
            .map(|k| TransformKeyframe {
                frame: k.frame,
                x: k.x,
                y: k.y,
                width: k.width,
                height: k.height,
                opacity: k.opacity.unwrap_or(1.0),
                rotation: k.rotation,
            })
            .collect();
        let descricao = format!("animar transformação do clipe {} ({} keyframes)", params.clip_ref, keyframes.len());
        let resultado = self
            .with_session_mut(|session| {
                session.snapshot_before(descricao);
                let filter_id = document::clip_animate_transform(&mut session.doc, &track_id, indice, &keyframes, interp)?;
                session.mark_modified();
                Ok(filter_id)
            })
            .await;
        match resultado {
            Ok(filter_id) => json!({
                "ok": true,
                "clip_ref": params.clip_ref,
                "filter_id": filter_id,
                "keyframes": keyframes.len(),
            }),
            Err(valor_erro) => valor_erro,
        }
    }

    pub(crate) async fn keyframe_set_many_impl(&self, params: KeyframeSetManyParams) -> Value {
        let (track_id, indice) = match parse_clip_ref(&params.clip_ref) {
            Ok(v) => v,
            Err(e) => return e,
        };
        let pares: Vec<(i64, String)> = params.keyframes.iter().map(|k| (k.frame, k.value.clone())).collect();
        let descricao = format!(
            "definir {} keyframes de {} no filtro {} do clipe {}",
            pares.len(),
            params.param,
            params.filter_id,
            params.clip_ref
        );
        let resultado = self
            .with_session_mut(|session| {
                session.snapshot_before(descricao);
                let total = document::keyframes_set_many(
                    &mut session.doc,
                    &track_id,
                    indice,
                    &params.filter_id,
                    &params.param,
                    &pares,
                    params.relative_to_clip.unwrap_or(false),
                    params.replace.unwrap_or(false),
                )?;
                session.mark_modified();
                Ok(total)
            })
            .await;
        match resultado {
            Ok(total) => json!({
                "ok": true,
                "clip_ref": params.clip_ref,
                "filter_id": params.filter_id,
                "param": params.param,
                "gravados": pares.len(),
                "total_keyframes": total,
            }),
            Err(valor_erro) => valor_erro,
        }
    }
}
