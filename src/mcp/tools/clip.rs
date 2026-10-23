//! `clip_get_properties`, `clip_set_speed`, `clip_reverse`,
//! `clip_set_opacity`, `audio_set_levels`, `audio_normalize`,
//! `clip_set_transform` (ferramentas de **clipe da timeline**, por
//! `clip_ref` — diferente de `clip_set_color`/`clip_add_note`, que são de
//! bin e ficam em `tools/bin.rs`).

use serde_json::{Value, json};

use crate::mcp::ffprobe::{probe_speech_intervals, probe_video_size};
use crate::mcp::params::{
    AudioDuckParams, AudioNormalizeParams, AudioSetLevelsParams, ClipRefParams, ClipReframeVerticalParams, ClipSetOpacityParams,
    ClipSetSpeedParams, ClipSetTransformParams,
};
use crate::model::{duck, reframe};
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

    /// Ducking: abaixa o clipe (ou a trilha) alvo enquanto a trilha da voz
    /// tem som. Os trechos de fala vêm dos clipes da trilha da voz (ou, com
    /// `detect_speech`, das partes não silenciosas deles, medidas com o
    /// `ffmpeg`); o resultado são keyframes do `level` de um filtro `volume`.
    pub(crate) async fn audio_duck_impl(&self, params: AudioDuckParams) -> Value {
        if params.clip_ref.is_some() == params.track_id.is_some() {
            return erro("informe exatamente um entre clip_ref (um clipe) e track_id (uma trilha inteira) como alvo");
        }
        if !(params.duck_db.is_finite() && params.duck_db != 0.0) {
            return erro("duck_db precisa ser um número diferente de zero (ex. 12 para abaixar 12 dB)");
        }
        let alvo_clip = match params.clip_ref.as_deref().map(parse_clip_ref).transpose() {
            Ok(v) => v,
            Err(e) => return e,
        };
        let trilha_alvo = alvo_clip.as_ref().map(|(t, _)| t.clone()).or_else(|| params.track_id.clone()).unwrap_or_default();
        if trilha_alvo == params.voice_track_id {
            return erro("a trilha da voz e o alvo não podem ser a mesma trilha");
        }
        let ducking = duck::Parametros::novo(params.duck_db, params.attack_frames, params.release_frames);

        // 1) clipes da voz (posição, mídia, velocidade) e fps, direto do documento
        struct ClipeVoz {
            posicao: i64,
            quadros: i64,
            inicio_midia: i64,
            recurso: Option<String>,
            velocidade: f64,
        }
        let leitura = self
            .with_session(|session| {
                let doc = &session.doc;
                if document::find_track_playlist(doc, &params.voice_track_id).is_none() {
                    return Err(format!("trilha da voz não encontrada: {}", params.voice_track_id));
                }
                if document::find_track_playlist(doc, &trilha_alvo).is_none() {
                    return Err(format!("trilha alvo não encontrada: {trilha_alvo}"));
                }
                let voz = document::list_clips_with_position(doc, &params.voice_track_id)
                    .into_iter()
                    .map(|(indice, producer_id, in_f, out_f, posicao)| {
                        let produtor = document::find_producer(doc, &producer_id);
                        let recurso = produtor.and_then(|p| {
                            document::get_property(p, "kdenlive-mcp-plus:original_resource")
                                .or_else(|| document::get_property(p, "resource"))
                        });
                        ClipeVoz {
                            posicao,
                            quadros: out_f - in_f + 1,
                            inicio_midia: in_f,
                            recurso,
                            velocidade: document::clip_speed(doc, &params.voice_track_id, indice),
                        }
                    })
                    .collect::<Vec<_>>();
                Ok((voz, document::profile_fps(doc)))
            })
            .await;
        let (clipes_voz, fps) = match leitura {
            Ok(Ok(v)) => v,
            Ok(Err(mensagem)) => return erro(mensagem),
            Err(valor_erro) => return valor_erro,
        };
        if clipes_voz.is_empty() {
            return erro(format!("a trilha da voz ({}) não tem nenhum clipe", params.voice_track_id));
        }

        // 2) trechos de fala, em quadros da timeline (fora do lock: ffmpeg é um subprocesso)
        let mut falas: Vec<(i64, i64)> = Vec::new();
        for clipe in &clipes_voz {
            if !params.detect_speech {
                falas.push((clipe.posicao, clipe.posicao + clipe.quadros));
                continue;
            }
            if (clipe.velocidade - 1.0).abs() > f64::EPSILON {
                return erro("detect_speech não funciona em clipes da voz com velocidade alterada; use detect_speech=false");
            }
            let Some(recurso) = &clipe.recurso else {
                return erro("um clipe da voz não tem arquivo de mídia para analisar; use detect_speech=false");
            };
            let detectadas = match probe_speech_intervals(
                std::path::Path::new(recurso),
                clipe.inicio_midia as f64 / fps,
                clipe.quadros as f64 / fps,
                params.silence_threshold_db,
                params.min_silence_seconds,
            )
            .await
            {
                Ok(v) => v,
                Err(e) => return erro(e),
            };
            for (ini, fim) in detectadas {
                falas.push((
                    clipe.posicao + (ini * fps).round() as i64,
                    clipe.posicao + (fim * fps).round() as i64,
                ));
            }
        }
        let trechos = duck::unir_trechos(falas, &ducking);

        // 3) aplica keyframes em cada clipe alvo
        let descricao = format!("ducking de áudio na trilha {trilha_alvo}");
        let resultado = self
            .with_session_mut(|session| {
                session.snapshot_before(descricao);
                let alvos: Vec<(usize, i64, i64)> = document::list_clips_with_position(&session.doc, &trilha_alvo)
                    .into_iter()
                    .filter(|(indice, ..)| alvo_clip.as_ref().is_none_or(|(_, i)| i == indice))
                    .map(|(indice, _, in_f, out_f, posicao)| (indice, posicao, out_f - in_f + 1))
                    .collect();
                if alvos.is_empty() {
                    return Err(crate::model::ModelError::NotFound(match &alvo_clip {
                        Some((t, i)) => format!("clipe não encontrado: {t}:{i}"),
                        None => format!("a trilha alvo ({trilha_alvo}) não tem nenhum clipe"),
                    }));
                }
                let mut aplicados = Vec::new();
                for (indice, posicao, quadros) in alvos {
                    let kfs = duck::keyframes(posicao, quadros, &trechos, &ducking);
                    let filter_id = document::set_ducking(&mut session.doc, &trilha_alvo, indice, &kfs)?;
                    aplicados.push(json!({
                        "clip_ref": format!("{trilha_alvo}:{indice}"),
                        "filter_id": filter_id,
                        "keyframes": kfs.iter().map(|(f, db)| json!([f, db])).collect::<Vec<_>>(),
                        "ducked": !kfs.is_empty(),
                    }));
                }
                session.mark_modified();
                Ok(aplicados)
            })
            .await;
        match resultado {
            Ok(alvos) => json!({
                "ok": true,
                "voice_track_id": params.voice_track_id,
                "duck_db": ducking.duck_db,
                "attack_frames": ducking.attack,
                "release_frames": ducking.release,
                "detect_speech": params.detect_speech,
                "speech_intervals_frames": trechos.iter().map(|(a, b)| json!([a, b])).collect::<Vec<_>>(),
                "targets": alvos,
            }),
            Err(valor_erro) => valor_erro,
        }
    }
}
