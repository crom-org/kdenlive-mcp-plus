//! `title_add`, `title_update`.
//!
//! Títulos nativos do Kdenlive (produtor `kdenlivetitle`, ver
//! `model::document::add_title_clip`), com a fonte resolvida para uma
//! família instalada (`mcp::fonts`).

use serde_json::{Value, json};

use crate::mcp::fonts::resolve_font_family;
use crate::mcp::params::{TitleAddParams, TitleUpdateParams};
use crate::mcp::server::{KdenliveMcpServer, erro};
use crate::model::document;

impl KdenliveMcpServer {
    pub(crate) async fn title_add_impl(&self, params: TitleAddParams) -> Value {
        let descricao = format!("criar título \"{}\"", params.text);
        let align = match params.align.as_deref().unwrap_or("center") {
            "left" => 1,
            "right" => 2,
            "center" => 4,
            outro => return erro(format!("align inválido: \"{outro}\". Use \"left\", \"center\" ou \"right\"")),
        };
        let cor = |c: &Option<String>| -> Result<Option<String>, Value> {
            c.as_deref().map(document::title_color).transpose().map_err(|e| erro(e.to_string()))
        };
        let (contorno, fundo) = match (cor(&params.outline_color), cor(&params.background_color)) {
            (Ok(a), Ok(b)) => (a, b),
            (Err(e), _) | (_, Err(e)) => return e,
        };
        let estilo = document::TitleStyle {
            bold: params.bold.unwrap_or(false),
            align,
            x: params.x,
            y: params.y,
            box_width: params.box_width,
            outline_width: params.outline_width.unwrap_or(0.0),
            outline_rgba: contorno,
            background_rgba: fundo,
            padding: params.padding.unwrap_or(24).max(0),
        };
        let familia = resolve_font_family(&params.font_family).await;
        let resultado = self
            .with_session_mut(|session| {
                session.snapshot_before(descricao);
                let id = session.alloc_producer_id();
                document::add_title_clip(
                    &mut session.doc,
                    &id,
                    &params.text,
                    params.duration_frames,
                    &familia,
                    params.font_size,
                    &params.color,
                    estilo,
                )?;
                session.mark_modified();
                Ok(id)
            })
            .await;
        match resultado {
            Ok(id) => json!({ "ok": true, "clip_id": id, "text": params.text }),
            Err(valor_erro) => valor_erro,
        }
    }

    pub(crate) async fn title_update_impl(&self, params: TitleUpdateParams) -> Value {
        let descricao = format!("atualizar título \"{}\"", params.clip_id);
        let familia = match &params.font_family {
            Some(f) => Some(resolve_font_family(f).await),
            None => None,
        };
        let resultado = self
            .with_session_mut(|session| {
                session.snapshot_before(descricao);
                document::update_title_clip(
                    &mut session.doc,
                    &params.clip_id,
                    params.text.as_deref(),
                    familia.as_deref(),
                    params.font_size,
                    params.color.as_deref(),
                )?;
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
