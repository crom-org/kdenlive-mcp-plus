//! `clip_get_properties`, `clip_set_speed`, `clip_reverse`,
//! `clip_set_opacity`, `audio_set_levels`, `audio_normalize`,
//! `clip_set_transform` (ferramentas de **clipe da timeline**, por
//! `clip_ref` — diferente de `clip_set_color`/`clip_add_note`, que são de
//! bin e ficam em `tools/bin.rs`).

use serde_json::{Value, json};

use crate::mcp::ffprobe::probe_video_size;
use crate::mcp::params::{
    AudioNormalizeParams, AudioSetLevelsParams, ClipRefParams, ClipReframeVerticalParams, ClipSetOpacityParams,
    ClipSetSpeedParams, ClipSetTransformParams,
};
use crate::model::reframe;
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

    /// Reenquadra um clipe de vídeo/imagem para o quadro do projeto (que
    /// precisa ser vertical): mede o vídeo com o `ffprobe`, calcula o
    /// retângulo (`model::reframe`) e o aplica pelo mesmo filtro `qtblend`
    /// de `clip_set_transform`.
    pub(crate) async fn clip_reframe_vertical_impl(&self, params: ClipReframeVerticalParams) -> Value {
        let (track_id, indice) = match parse_clip_ref(&params.clip_ref) {
            Ok(v) => v,
            Err(e) => return e,
        };
        let modo = match params.mode.parse::<reframe::Modo>() {
            Ok(m) => m,
            Err(e) => return erro(e),
        };

        // 1) o que o documento diz: o arquivo de mídia do clipe e o quadro do projeto
        let leitura = self
            .with_session(|session| {
                let entry = document::get_clip(&session.doc, &track_id, indice)?;
                let producer = document::find_producer(&session.doc, entry.attributes.get("producer")?)?;
                // um clipe com velocidade alterada fica dentro de um `timewarp`:
                // a mídia de verdade está guardada à parte
                let recurso = document::get_property(producer, "kdenlive-mcp-plus:original_resource")
                    .or_else(|| document::get_property(producer, "resource"))?;
                let perfil = document::profile_element(&session.doc)?;
                let medida = |n: &str| perfil.attributes.get(n).and_then(|v| v.parse::<f64>().ok());
                Some((recurso, (medida("width")?, medida("height")?)))
            })
            .await;
        let (recurso, quadro) = match leitura {
            Ok(Some(v)) => v,
            Ok(None) => return erro(format!("clipe não encontrado: {}", params.clip_ref)),
            Err(valor_erro) => return valor_erro,
        };
        if quadro.1 <= quadro.0 {
            return erro(format!(
                "o quadro do projeto é {}x{}, que não é vertical; troque o perfil com project_set_profile \
                 (ex. \"vertical_hd_30\") antes de reenquadrar",
                quadro.0, quadro.1
            ));
        }

        // 2) o tamanho do vídeo (fora do lock: é um subprocesso)
        let origem = match probe_video_size(std::path::Path::new(&recurso)).await {
            Ok(v) => v,
            Err(e) => return erro(e),
        };
        let retangulo = match reframe::calcular(origem, quadro, modo, params.focus_x, params.focus_y) {
            Ok(r) => r,
            Err(e) => return erro(e),
        };

        // 3) aplica pelo `qtblend`
        let descricao = format!("reenquadrar o clipe {} para o quadro do projeto", params.clip_ref);
        let resultado = self
            .with_session_mut(|session| {
                session.snapshot_before(descricao);
                let filter_id = document::set_clip_transform(
                    &mut session.doc,
                    &track_id,
                    indice,
                    retangulo.x,
                    retangulo.y,
                    retangulo.largura,
                    retangulo.altura,
                    None,
                )?;
                session.mark_modified();
                Ok(filter_id)
            })
            .await;
        match resultado {
            Ok(filter_id) => json!({
                "ok": true,
                "clip_ref": params.clip_ref,
                "filter_id": filter_id,
                "mode": params.mode,
                "source_size": { "width": origem.0, "height": origem.1 },
                "frame_size": { "width": quadro.0, "height": quadro.1 },
                "rect": {
                    "x": retangulo.x, "y": retangulo.y,
                    "width": retangulo.largura, "height": retangulo.altura,
                },
            }),
            Err(valor_erro) => valor_erro,
        }
    }
}
