//! `bin_import_clip`, `bin_list_clips`, `bin_get_clip_details`,
//! `bin_remove_clip`, `clip_set_color`, `clip_add_note` (ferramentas de
//! bin, apesar do prefixo `clip_` no nome original — ver docs/04).

use std::path::{Path, PathBuf};

use serde_json::{Value, json};

use crate::mcp::ffprobe;
use crate::mcp::params::{
    BinClipColorParams, BinClipIdParams, BinClipNoteParams, BinCreateFolderParams, BinImportClipParams,
    BinImportFolderParams, BinMoveClipParams, BinRenameParams,
};
use crate::mcp::server::{KdenliveMcpServer, erro};
use crate::model::document;

fn clip_json(doc: &xmltree::Element, id: &str) -> Option<Value> {
    let producer = document::find_producer(doc, id)?;
    let recurso = document::get_property(producer, "resource");
    let existe = recurso.as_deref().map(|r| Path::new(r).exists());
    Some(json!({
        "clip_id": id,
        "nome": document::get_property(producer, "kdenlive:clipname"),
        "resource": recurso,
        "cor": document::get_property(producer, "kdenlive-mcp-plus:color"),
        "nota": document::get_property(producer, "kdenlive-mcp-plus:note"),
        "in_frame": producer.attributes.get("in").and_then(|v| v.parse::<i64>().ok()).unwrap_or(0),
        "out_frame": producer.attributes.get("out").and_then(|v| v.parse::<i64>().ok()).unwrap_or(0),
        "arquivo_existe": existe,
    }))
}

/// Extensões de mídia reconhecidas por `bin_import_folder` (não recursivo
/// nesta fase).
const EXTENSOES_MIDIA: &[&str] = &[
    "mp4", "mov", "mkv", "webm", "avi", "mxf", "wav", "mp3", "flac", "ogg", "m4a", "png", "jpg",
    "jpeg",
];

impl KdenliveMcpServer {
    /// Lógica compartilhada por `bin_import_clip` e `bin_import_folder`.
    async fn importar_um_clipe(&self, caminho: &Path, nome: Option<String>) -> Value {
        let fps = match self.with_session(|s| document::profile_fps(&s.doc)).await {
            Ok(fps) => fps,
            Err(valor_erro) => return valor_erro,
        };

        let quadros = ffprobe::probe_duration_frames(caminho, fps).await;
        let (in_frame, out_frame) = match quadros {
            Some(n) if n > 0 => (0, n - 1),
            _ => (0, 0),
        };
        let nota = quadros.is_none().then(|| {
            "duração não sondada (ffprobe indisponível ou falhou); in/out ficaram em 0"
                .to_string()
        });

        let nome = nome.unwrap_or_else(|| {
            caminho
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_else(|| caminho.display().to_string())
        });
        let resource = caminho.display().to_string();
        let descricao = format!("importar clipe \"{nome}\"");

        let resultado = self
            .with_session_mut(|session| {
                session.snapshot_before(descricao);
                let id = session.alloc_producer_id();
                document::add_bin_clip(&mut session.doc, &id, &resource, &nome, in_frame, out_frame)?;
                session.mark_modified();
                Ok(id)
            })
            .await;

        match resultado {
            Ok(id) => json!({
                "ok": true,
                "clip_id": id,
                "in_frame": in_frame,
                "out_frame": out_frame,
                "nota": nota,
            }),
            Err(valor_erro) => valor_erro,
        }
    }

    pub(crate) async fn bin_import_clip_impl(&self, params: BinImportClipParams) -> Value {
        let caminho = PathBuf::from(&params.path);
        if !caminho.exists() {
            return erro(format!("arquivo de mídia não encontrado: {}", caminho.display()));
        }
        self.importar_um_clipe(&caminho, params.name.clone()).await
    }

    pub(crate) async fn bin_import_folder_impl(&self, params: BinImportFolderParams) -> Value {
        let pasta = PathBuf::from(&params.path);
        if !pasta.is_dir() {
            return erro(format!("pasta não encontrada: {}", pasta.display()));
        }

        let entradas = match std::fs::read_dir(&pasta) {
            Ok(it) => it,
            Err(e) => return erro(format!("falha ao ler a pasta {}: {e}", pasta.display())),
        };

        let mut arquivos: Vec<PathBuf> = entradas
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .filter(|p| {
                p.is_file()
                    && p.extension()
                        .and_then(|ext| ext.to_str())
                        .is_some_and(|ext| EXTENSOES_MIDIA.iter().any(|e| e.eq_ignore_ascii_case(ext)))
            })
            .collect();
        arquivos.sort();

        if arquivos.is_empty() {
            return erro(format!(
                "nenhum arquivo de mídia reconhecido em {} (extensões aceitas: {})",
                pasta.display(),
                EXTENSOES_MIDIA.join(", ")
            ));
        }

        let mut importados = Vec::with_capacity(arquivos.len());
        let mut falhas = Vec::new();
        for arquivo in arquivos {
            let resultado = self.importar_um_clipe(&arquivo, None).await;
            if resultado["ok"] == json!(true) {
                importados.push(resultado);
            } else {
                falhas.push(json!({ "arquivo": arquivo.display().to_string(), "erro": resultado["erro"] }));
            }
        }

        json!({ "ok": true, "importados": importados, "falhas": falhas })
    }

    pub(crate) async fn bin_list_clips_impl(&self) -> Value {
        let resultado = self
            .with_session(|session| {
                document::bin_clip_ids(&session.doc)
                    .into_iter()
                    .filter_map(|id| clip_json(&session.doc, &id))
                    .collect::<Vec<_>>()
            })
            .await;
        match resultado {
            Ok(lista) => json!({ "ok": true, "clipes": lista }),
            Err(valor_erro) => valor_erro,
        }
    }

    pub(crate) async fn bin_get_clip_details_impl(&self, params: BinClipIdParams) -> Value {
        let resultado = self
            .with_session(|session| clip_json(&session.doc, &params.clip_id))
            .await;
        match resultado {
            Ok(Some(mut detalhes)) => {
                detalhes["ok"] = json!(true);
                detalhes
            }
            Ok(None) => erro(format!("clipe não encontrado: {}", params.clip_id)),
            Err(valor_erro) => valor_erro,
        }
    }

    pub(crate) async fn bin_remove_clip_impl(&self, params: BinClipIdParams) -> Value {
        let descricao = format!("remover clipe \"{}\" da bin", params.clip_id);
        let resultado = self
            .with_session_mut(|session| {
                session.snapshot_before(descricao);
                document::remove_bin_clip(&mut session.doc, &params.clip_id)?;
                session.mark_modified();
                Ok(())
            })
            .await;
        match resultado {
            Ok(()) => json!({ "ok": true, "clip_id": params.clip_id }),
            Err(valor_erro) => valor_erro,
        }
    }

    pub(crate) async fn clip_set_color_impl(&self, params: BinClipColorParams) -> Value {
        let descricao = format!("definir cor do clipe \"{}\"", params.clip_id);
        let resultado = self
            .with_session_mut(|session| {
                session.snapshot_before(descricao);
                document::bin_clip_set_color(&mut session.doc, &params.clip_id, &params.color)?;
                session.mark_modified();
                Ok(())
            })
            .await;
        match resultado {
            Ok(()) => json!({ "ok": true, "clip_id": params.clip_id, "cor": params.color }),
            Err(valor_erro) => valor_erro,
        }
    }

    pub(crate) async fn clip_add_note_impl(&self, params: BinClipNoteParams) -> Value {
        let descricao = format!("definir nota do clipe \"{}\"", params.clip_id);
        let resultado = self
            .with_session_mut(|session| {
                session.snapshot_before(descricao);
                document::bin_clip_set_note(&mut session.doc, &params.clip_id, &params.note)?;
                session.mark_modified();
                Ok(())
            })
            .await;
        match resultado {
            Ok(()) => json!({ "ok": true, "clip_id": params.clip_id, "nota": params.note }),
            Err(valor_erro) => valor_erro,
        }
    }

    pub(crate) async fn bin_create_folder_impl(&self, params: BinCreateFolderParams) -> Value {
        let descricao = format!("criar pasta \"{}\" na bin", params.name);
        let resultado = self
            .with_session_mut(|session| {
                session.snapshot_before(descricao);
                let id = document::create_bin_folder(&mut session.doc, params.parent_folder_id.as_deref(), &params.name)?;
                session.mark_modified();
                Ok(id)
            })
            .await;
        match resultado {
            Ok(folder_id) => json!({ "ok": true, "folder_id": folder_id, "name": params.name }),
            Err(valor_erro) => valor_erro,
        }
    }

    pub(crate) async fn bin_move_clip_impl(&self, params: BinMoveClipParams) -> Value {
        let descricao = format!("mover clipe \"{}\" na bin", params.clip_id);
        let resultado = self
            .with_session_mut(|session| {
                session.snapshot_before(descricao);
                document::bin_move_clip(&mut session.doc, &params.clip_id, params.folder_id.as_deref())?;
                session.mark_modified();
                Ok(())
            })
            .await;
        match resultado {
            Ok(()) => json!({ "ok": true, "clip_id": params.clip_id, "folder_id": params.folder_id }),
            Err(valor_erro) => valor_erro,
        }
    }

    pub(crate) async fn bin_rename_impl(&self, params: BinRenameParams) -> Value {
        let descricao = format!("renomear clipe \"{}\" para \"{}\"", params.clip_id, params.name);
        let resultado = self
            .with_session_mut(|session| {
                session.snapshot_before(descricao);
                document::bin_clip_rename(&mut session.doc, &params.clip_id, &params.name)?;
                session.mark_modified();
                Ok(())
            })
            .await;
        match resultado {
            Ok(()) => json!({ "ok": true, "clip_id": params.clip_id, "name": params.name }),
            Err(valor_erro) => valor_erro,
        }
    }
}
