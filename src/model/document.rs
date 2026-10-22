//! Construção e consulta de um documento MLT XML em memória.
//!
//! Convenções deste projeto (documentadas aqui porque não são universais do
//! MLT nem garantidamente idênticas ao que o Kdenlive de verdade grava —
//! ver docs/09-riscos-e-bugs.md):
//!
//! - Um único `<tractor>` por documento (a sequência principal), sempre o
//!   **último** filho de `<mlt>`. Múltiplas sequências (recurso do Kdenlive
//!   23+) não são suportadas nesta fase.
//! - Cada trilha da timeline é uma `<playlist>` referenciada por um
//!   `<track producer="...">` dentro do tractor principal.
//! - A bin do projeto é a `<playlist id="main_bin">` — id confirmado no
//!   `dev-docs/fileformat.md` do próprio repositório do Kdenlive (ver
//!   docs/decisoes.md). Suas `<entry>` apontam para os `<producer>` ainda
//!   não colocados em nenhuma trilha.
//! - Uma **referência de clipe**, nas ferramentas deste servidor, é a
//!   string `"<id da trilha>:<índice da entrada>"` (ex. `"playlist0:2"`).
//!   Não é um conceito do MLT nem do Kdenlive: é só como este servidor
//!   localiza um clipe de volta depois de listá-lo.
//! - Inserir um clipe numa posição além do fim atual da trilha cria um
//!   `<blank length="..."/>` de preenchimento; inserir numa posição que
//!   sobreporia um clipe existente é um erro nesta fase (sem "ripple
//!   insert" ainda).

use serde::{Deserialize, Serialize};
use xmltree::{Element, EmitterConfig, XMLNode};

use crate::model::profile;

pub const MAIN_BIN_ID: &str = "main_bin";
pub const MAIN_TRACTOR_ID: &str = "tractor0";

#[derive(Debug, thiserror::Error)]
pub enum ModelError {
    #[error("XML malformado: {0}")]
    Parse(String),
    #[error("falha ao gerar XML: {0}")]
    Write(String),
    #[error("{0}")]
    NotFound(String),
    #[error("{0}")]
    Invalid(String),
    #[error("falha de E/S: {0}")]
    Io(String),
}

pub fn parse(xml_text: &str) -> Result<Element, ModelError> {
    Element::parse(xml_text.as_bytes()).map_err(|e| ModelError::Parse(e.to_string()))
}

pub fn to_string_pretty(root: &Element) -> Result<String, ModelError> {
    let mut buf = Vec::new();
    let config = EmitterConfig::new()
        .perform_indent(true)
        .write_document_declaration(true);
    root.write_with_config(&mut buf, config)
        .map_err(|e| ModelError::Write(e.to_string()))?;
    String::from_utf8(buf).map_err(|e| ModelError::Write(e.to_string()))
}

/// Cria um documento novo e mínimo: perfil, bin vazia e um tractor vazio.
pub fn new_document(profile_name: &str) -> Result<Element, ModelError> {
    let profile = profile::find(profile_name).ok_or_else(|| {
        ModelError::Invalid(format!(
            "perfil desconhecido: \"{profile_name}\". Use project_list_profiles para ver os \
             perfis válidos."
        ))
    })?;

    let mut root = Element::new("mlt");
    root.attributes.insert("version".to_string(), "7.22.0".to_string());
    root.attributes.insert("producer".to_string(), MAIN_BIN_ID.to_string());

    let mut profile_el = Element::new("profile");
    for (k, v) in profile.to_xml_attrs() {
        profile_el.attributes.insert(k, v);
    }
    root.children.push(XMLNode::Element(profile_el));

    let mut main_bin = Element::new("playlist");
    main_bin.attributes.insert("id".to_string(), MAIN_BIN_ID.to_string());
    root.children.push(XMLNode::Element(main_bin));

    let mut tractor = Element::new("tractor");
    tractor.attributes.insert("id".to_string(), MAIN_TRACTOR_ID.to_string());
    root.children.push(XMLNode::Element(tractor));

    Ok(root)
}

// ---------------------------------------------------------------------------
// Acesso a elementos de topo
// ---------------------------------------------------------------------------

pub fn profile_element(root: &Element) -> Option<&Element> {
    root.get_child("profile")
}

pub fn profile_element_mut(root: &mut Element) -> Option<&mut Element> {
    root.get_mut_child("profile")
}

/// Taxa de quadros do perfil do documento (25 fps se o perfil não puder ser
/// lido — nunca deveria acontecer num documento criado por este projeto).
pub fn profile_fps(root: &Element) -> f64 {
    let Some(p) = profile_element(root) else { return 25.0 };
    let num: f64 = p.attributes.get("frame_rate_num").and_then(|v| v.parse().ok()).unwrap_or(25.0);
    let den: f64 = p.attributes.get("frame_rate_den").and_then(|v| v.parse().ok()).unwrap_or(1.0);
    if den == 0.0 { num } else { num / den }
}

fn attr_eq(el: &Element, tag: &str, attr: &str, value: &str) -> bool {
    el.name == tag && el.attributes.get(attr).map(String::as_str) == Some(value)
}

pub fn find_playlist<'a>(root: &'a Element, id: &str) -> Option<&'a Element> {
    root.children
        .iter()
        .filter_map(XMLNode::as_element)
        .find(|e| attr_eq(e, "playlist", "id", id))
}

pub fn find_playlist_mut<'a>(root: &'a mut Element, id: &str) -> Option<&'a mut Element> {
    root.children
        .iter_mut()
        .filter_map(XMLNode::as_mut_element)
        .find(|e| attr_eq(e, "playlist", "id", id))
}

pub fn find_producer<'a>(root: &'a Element, id: &str) -> Option<&'a Element> {
    root.children
        .iter()
        .filter_map(XMLNode::as_element)
        .find(|e| attr_eq(e, "producer", "id", id))
}

pub fn find_producer_mut<'a>(root: &'a mut Element, id: &str) -> Option<&'a mut Element> {
    root.children
        .iter_mut()
        .filter_map(XMLNode::as_mut_element)
        .find(|e| attr_eq(e, "producer", "id", id))
}

pub fn main_bin(root: &Element) -> Option<&Element> {
    find_playlist(root, MAIN_BIN_ID)
}

pub fn main_bin_mut(root: &mut Element) -> Option<&mut Element> {
    find_playlist_mut(root, MAIN_BIN_ID)
}

/// O tractor principal: o último `<tractor>` entre os filhos de `<mlt>`.
pub fn main_tractor(root: &Element) -> Option<&Element> {
    root.children
        .iter()
        .rev()
        .filter_map(XMLNode::as_element)
        .find(|e| e.name == "tractor")
}

pub fn main_tractor_mut(root: &mut Element) -> Option<&mut Element> {
    root.children
        .iter_mut()
        .rev()
        .filter_map(XMLNode::as_mut_element)
        .find(|e| e.name == "tractor")
}

/// Maior número já usado num id com o prefixo dado, mais um (ex. se existem
/// `producer0` e `producer2`, devolve 3). Olha só os elementos de tag `tag`
/// entre os filhos diretos de `root`.
///
/// Isto só serve para **inicializar** o contador de uma sessão nova (ver
/// `Session`): uma vez inicializado, o contador da sessão nunca mais
/// rescaneia o documento, para nunca reaproveitar o id de algo que já foi
/// removido durante a mesma sessão.
pub fn next_available_n(root: &Element, tag: &str, prefix: &str) -> u64 {
    let mut max_n: i64 = -1;
    for child in root.children.iter().filter_map(XMLNode::as_element) {
        if child.name == tag
            && let Some(id) = child.attributes.get("id")
            && let Some(num) = id.strip_prefix(prefix).and_then(|s| s.parse::<i64>().ok())
        {
            max_n = max_n.max(num);
        }
    }
    (max_n + 1) as u64
}

// ---------------------------------------------------------------------------
// Propriedades (`<property name="...">valor</property>`)
// ---------------------------------------------------------------------------

pub fn get_property(el: &Element, name: &str) -> Option<String> {
    el.children
        .iter()
        .filter_map(XMLNode::as_element)
        .find(|c| attr_eq(c, "property", "name", name))
        .and_then(|c| c.get_text())
        .map(|cow| cow.into_owned())
}

pub fn set_property(el: &mut Element, name: &str, value: &str) {
    if let Some(existing) = el
        .children
        .iter_mut()
        .filter_map(XMLNode::as_mut_element)
        .find(|c| attr_eq(c, "property", "name", name))
    {
        existing.children = vec![XMLNode::Text(value.to_string())];
    } else {
        let mut prop = Element::new("property");
        prop.attributes.insert("name".to_string(), name.to_string());
        prop.children.push(XMLNode::Text(value.to_string()));
        el.children.push(XMLNode::Element(prop));
    }
}

pub fn remove_property(el: &mut Element, name: &str) {
    el.children.retain(|c| !matches!(c, XMLNode::Element(e) if attr_eq(e, "property", "name", name)));
}

// ---------------------------------------------------------------------------
// Bin
// ---------------------------------------------------------------------------

/// Importa um arquivo de mídia como um novo produtor na bin do projeto, com
/// o id `id` (alocado por `Session::alloc_producer_id`, nunca gerado aqui —
/// ver o comentário de `next_available_n`). `in_frame`/`out_frame` são o
/// intervalo utilizável do clipe (0-based, inclusive); passe `(0, 0)`
/// quando a duração não puder ser sondada.
pub fn add_bin_clip(
    root: &mut Element,
    id: &str,
    resource: &str,
    clip_name: &str,
    in_frame: i64,
    out_frame: i64,
) -> Result<String, ModelError> {
    let id = id.to_string();

    let mut producer = Element::new("producer");
    producer.attributes.insert("id".to_string(), id.clone());
    producer.attributes.insert("in".to_string(), in_frame.to_string());
    producer.attributes.insert("out".to_string(), out_frame.to_string());
    set_property(&mut producer, "resource", resource);
    set_property(&mut producer, "mlt_service", "avformat");
    set_property(&mut producer, "kdenlive:clipname", clip_name);
    set_property(&mut producer, "kdenlive:id", &id);

    let insert_at = root
        .children
        .iter()
        .position(|n| matches!(n, XMLNode::Element(e) if attr_eq(e, "playlist", "id", MAIN_BIN_ID)))
        .unwrap_or(root.children.len());
    root.children.insert(insert_at, XMLNode::Element(producer));

    let bin = main_bin_mut(root)
        .ok_or_else(|| ModelError::NotFound(format!("playlist \"{MAIN_BIN_ID}\" não encontrada")))?;
    let mut entry = Element::new("entry");
    entry.attributes.insert("producer".to_string(), id.clone());
    bin.children.push(XMLNode::Element(entry));

    Ok(id)
}

/// Remove um produtor da bin: a entrada em `main_bin` e, se ele não estiver
/// referenciado em nenhuma trilha, o próprio `<producer>`.
pub fn remove_bin_clip(root: &mut Element, producer_id: &str) -> Result<(), ModelError> {
    {
        let bin = main_bin_mut(root)
            .ok_or_else(|| ModelError::NotFound(format!("playlist \"{MAIN_BIN_ID}\" não encontrada")))?;
        let before = bin.children.len();
        bin.children.retain(
            |n| !matches!(n, XMLNode::Element(e) if attr_eq(e, "entry", "producer", producer_id)),
        );
        if bin.children.len() == before {
            return Err(ModelError::NotFound(format!(
                "clipe \"{producer_id}\" não está na bin"
            )));
        }
    }

    let used_elsewhere = root
        .children
        .iter()
        .filter_map(XMLNode::as_element)
        .filter(|e| e.name == "playlist" && e.attributes.get("id").map(String::as_str) != Some(MAIN_BIN_ID))
        .flat_map(|p| p.children.iter())
        .filter_map(XMLNode::as_element)
        .any(|e| attr_eq(e, "entry", "producer", producer_id));

    if !used_elsewhere {
        root.children.retain(
            |n| !matches!(n, XMLNode::Element(e) if attr_eq(e, "producer", "id", producer_id)),
        );
    }

    Ok(())
}

/// Lista os ids de produtor que estão na bin (referenciados por `main_bin`).
pub fn bin_clip_ids(root: &Element) -> Vec<String> {
    main_bin(root)
        .map(|bin| {
            bin.children
                .iter()
                .filter_map(XMLNode::as_element)
                .filter(|e| e.name == "entry")
                .filter_map(|e| e.attributes.get("producer").cloned())
                .collect()
        })
        .unwrap_or_default()
}

/// Rótulo de cor de um clipe da bin. Convenção própria deste projeto (não
/// confirmada como igual à do Kdenlive real — a pesquisa só confirmou
/// `kdenlive:clipname`/`folderid`/`zone_in`/`zone_out`/`originalurl`/
/// `proxy`/`id`/`clip_type`/`file_size`/`file_hash`, nenhuma propriedade de
/// cor ou nota; ver docs/09-riscos-e-bugs.md).
pub fn bin_clip_set_color(root: &mut Element, producer_id: &str, color: &str) -> Result<(), ModelError> {
    let producer = find_producer_mut(root, producer_id)
        .ok_or_else(|| ModelError::NotFound(format!("clipe não encontrado na bin: {producer_id}")))?;
    set_property(producer, "kdenlive-mcp-plus:color", color);
    Ok(())
}

pub fn bin_clip_set_note(root: &mut Element, producer_id: &str, note: &str) -> Result<(), ModelError> {
    let producer = find_producer_mut(root, producer_id)
        .ok_or_else(|| ModelError::NotFound(format!("clipe não encontrado na bin: {producer_id}")))?;
    set_property(producer, "kdenlive-mcp-plus:note", note);
    Ok(())
}

// ---------------------------------------------------------------------------
// Trilhas e clipes da timeline
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrackKind {
    Video,
    Audio,
}

/// Cria uma trilha nova (uma `<playlist>` + um `<track>` no tractor
/// principal) com o id `id` (alocado por `Session::alloc_playlist_id`) e
/// devolve o id da trilha (= id da playlist).
pub fn add_track(root: &mut Element, id: &str, kind: TrackKind) -> Result<String, ModelError> {
    let playlist_id = id.to_string();

    let mut playlist = Element::new("playlist");
    playlist.attributes.insert("id".to_string(), playlist_id.clone());
    // O tipo da trilha fica guardado aqui, numa propriedade própria deste
    // projeto — não no atributo `hide` do `<track>` (que é só o estado de
    // visibilidade/áudio, alterável por track_set_hidden/track_set_muted,
    // sem relação com o tipo original da trilha).
    set_property(
        &mut playlist,
        "kdenlive-mcp-plus:kind",
        match kind {
            TrackKind::Video => "video",
            TrackKind::Audio => "audio",
        },
    );

    let insert_at = root
        .children
        .iter()
        .position(|n| matches!(n, XMLNode::Element(e) if e.name == "tractor"))
        .unwrap_or(root.children.len());
    root.children.insert(insert_at, XMLNode::Element(playlist));

    let tractor = main_tractor_mut(root)
        .ok_or_else(|| ModelError::NotFound("tractor principal não encontrado".to_string()))?;
    let mut track = Element::new("track");
    track.attributes.insert("producer".to_string(), playlist_id.clone());
    if kind == TrackKind::Audio {
        track.attributes.insert("hide".to_string(), "video".to_string());
    }
    tractor.children.push(XMLNode::Element(track));

    Ok(playlist_id)
}

/// Remove uma trilha (a `<track>` do tractor e a `<playlist>` correspondente,
/// com todos os seus clipes). Produtores referenciados só por essa trilha
/// voltam para a bin se ainda estiverem referenciados por `main_bin`;
/// nenhum produtor é apagado por esta operação.
pub fn remove_track(root: &mut Element, track_id: &str) -> Result<(), ModelError> {
    {
        let tractor = main_tractor_mut(root)
            .ok_or_else(|| ModelError::NotFound("tractor principal não encontrado".to_string()))?;
        let before = tractor.children.len();
        tractor
            .children
            .retain(|n| !matches!(n, XMLNode::Element(e) if attr_eq(e, "track", "producer", track_id)));
        if tractor.children.len() == before {
            return Err(ModelError::NotFound(format!("trilha não encontrada: {track_id}")));
        }
    }
    root.children
        .retain(|n| !matches!(n, XMLNode::Element(e) if attr_eq(e, "playlist", "id", track_id)));
    Ok(())
}

/// Lista os ids de trilha, na ordem em que aparecem no tractor principal
/// (primeiro = mais ao fundo, última = mais acima — convenção do MLT).
pub fn track_ids(root: &Element) -> Vec<String> {
    main_tractor(root)
        .map(|t| {
            t.children
                .iter()
                .filter_map(XMLNode::as_element)
                .filter(|e| e.name == "track")
                .filter_map(|e| e.attributes.get("producer").cloned())
                .collect()
        })
        .unwrap_or_default()
}

pub fn track_kind(root: &Element, track_id: &str) -> Option<TrackKind> {
    let playlist = find_playlist(root, track_id)?;
    match get_property(playlist, "kdenlive-mcp-plus:kind").as_deref() {
        Some("audio") => Some(TrackKind::Audio),
        _ => Some(TrackKind::Video),
    }
}

/// Componentes atualmente suprimidos de uma trilha: vídeo invisível e/ou
/// áudio mudo, lidos do atributo `hide` do `<track>` (`"video"`, `"audio"`
/// ou `"both"` — convenção do próprio MLT, independente do tipo da trilha).
fn hide_components(hide: Option<&str>) -> (bool, bool) {
    match hide {
        Some("video") => (true, false),
        Some("audio") => (false, true),
        Some("both") => (true, true),
        _ => (false, false),
    }
}

fn hide_value(video: bool, audio: bool) -> Option<&'static str> {
    match (video, audio) {
        (true, true) => Some("both"),
        (true, false) => Some("video"),
        (false, true) => Some("audio"),
        (false, false) => None,
    }
}

fn track_element_mut<'a>(root: &'a mut Element, track_id: &str) -> Option<&'a mut Element> {
    main_tractor_mut(root)?
        .children
        .iter_mut()
        .filter_map(XMLNode::as_mut_element)
        .find(|e| attr_eq(e, "track", "producer", track_id))
}

/// Oculta/mostra a trilha (vídeo), sem alterar o estado de mudo (áudio).
pub fn track_set_hidden(root: &mut Element, track_id: &str, hidden: bool) -> Result<(), ModelError> {
    let track = track_element_mut(root, track_id)
        .ok_or_else(|| ModelError::NotFound(format!("trilha não encontrada: {track_id}")))?;
    let (_, audio) = hide_components(track.attributes.get("hide").map(String::as_str));
    match hide_value(hidden, audio) {
        Some(valor) => track.attributes.insert("hide".to_string(), valor.to_string()),
        None => track.attributes.shift_remove("hide"),
    };
    Ok(())
}

/// Muta/desmuta a trilha (áudio), sem alterar a visibilidade (vídeo).
pub fn track_set_muted(root: &mut Element, track_id: &str, muted: bool) -> Result<(), ModelError> {
    let track = track_element_mut(root, track_id)
        .ok_or_else(|| ModelError::NotFound(format!("trilha não encontrada: {track_id}")))?;
    let (video, _) = hide_components(track.attributes.get("hide").map(String::as_str));
    match hide_value(video, muted) {
        Some(valor) => track.attributes.insert("hide".to_string(), valor.to_string()),
        None => track.attributes.shift_remove("hide"),
    };
    Ok(())
}

/// `(oculta, muda)` — estado atual de visibilidade/áudio da trilha.
pub fn track_hidden_muted(root: &Element, track_id: &str) -> Option<(bool, bool)> {
    let hide = main_tractor(root)?
        .children
        .iter()
        .filter_map(XMLNode::as_element)
        .find(|e| attr_eq(e, "track", "producer", track_id))?
        .attributes
        .get("hide")
        .map(String::as_str);
    Some(hide_components(hide))
}

/// Nome de exibição da trilha (guardado como propriedade na playlist).
pub fn track_name(root: &Element, track_id: &str) -> Option<String> {
    get_property(find_playlist(root, track_id)?, "kdenlive-mcp-plus:name")
}

pub fn track_set_name(root: &mut Element, track_id: &str, nome: &str) -> Result<(), ModelError> {
    let playlist = find_playlist_mut(root, track_id)
        .ok_or_else(|| ModelError::NotFound(format!("trilha não encontrada: {track_id}")))?;
    set_property(playlist, "kdenlive-mcp-plus:name", nome);
    Ok(())
}

/// Trava/destrava a trilha (conceito só de edição; não afeta o render).
pub fn track_set_locked(root: &mut Element, track_id: &str, locked: bool) -> Result<(), ModelError> {
    let playlist = find_playlist_mut(root, track_id)
        .ok_or_else(|| ModelError::NotFound(format!("trilha não encontrada: {track_id}")))?;
    set_property(playlist, "kdenlive-mcp-plus:locked", if locked { "1" } else { "0" });
    Ok(())
}

pub fn track_is_locked(root: &Element, track_id: &str) -> bool {
    find_playlist(root, track_id)
        .and_then(|p| get_property(p, "kdenlive-mcp-plus:locked"))
        .as_deref()
        == Some("1")
}

/// Move a trilha uma posição acima (troca com a anterior na ordem do
/// tractor). Erro se já for a primeira.
pub fn track_move_up(root: &mut Element, track_id: &str) -> Result<(), ModelError> {
    move_track(root, track_id, -1)
}

/// Move a trilha uma posição abaixo (troca com a próxima).
pub fn track_move_down(root: &mut Element, track_id: &str) -> Result<(), ModelError> {
    move_track(root, track_id, 1)
}

fn move_track(root: &mut Element, track_id: &str, delta: isize) -> Result<(), ModelError> {
    let tractor = main_tractor_mut(root)
        .ok_or_else(|| ModelError::NotFound("tractor principal não encontrado".to_string()))?;
    let indices: Vec<usize> = tractor
        .children
        .iter()
        .enumerate()
        .filter(|(_, n)| matches!(n, XMLNode::Element(e) if e.name == "track"))
        .map(|(i, _)| i)
        .collect();
    let posicao = indices
        .iter()
        .position(|&i| matches!(&tractor.children[i], XMLNode::Element(e) if attr_eq(e, "track", "producer", track_id)))
        .ok_or_else(|| ModelError::NotFound(format!("trilha não encontrada: {track_id}")))?;
    let nova_posicao = posicao as isize + delta;
    if nova_posicao < 0 || nova_posicao as usize >= indices.len() {
        return Err(ModelError::Invalid(format!(
            "a trilha \"{track_id}\" já está {}",
            if delta < 0 { "na primeira posição" } else { "na última posição" }
        )));
    }
    tractor.children.swap(indices[posicao], indices[nova_posicao as usize]);
    Ok(())
}

fn entry_length(node: &Element) -> i64 {
    match node.name.as_str() {
        "blank" => node.attributes.get("length").and_then(|s| s.parse().ok()).unwrap_or(0),
        "entry" => {
            let in_f: i64 = node.attributes.get("in").and_then(|s| s.parse().ok()).unwrap_or(0);
            let out_f: i64 = node.attributes.get("out").and_then(|s| s.parse().ok()).unwrap_or(0);
            (out_f - in_f + 1).max(0)
        }
        _ => 0,
    }
}

/// Duração total de uma trilha (playlist), em quadros.
pub fn track_length(playlist: &Element) -> i64 {
    playlist.children.iter().filter_map(XMLNode::as_element).map(entry_length).sum()
}

/// Posiciona um clipe (produtor já existente, da bin ou não) numa trilha.
/// Se `position` ficar depois do fim atual da trilha, um `<blank>` de
/// preenchimento é criado; se sobrepuser um clipe existente, é um erro.
/// Devolve o índice da entrada criada (para montar a referência de clipe
/// `"<track_id>:<índice>"`).
pub fn add_clip_to_track(
    root: &mut Element,
    track_id: &str,
    producer_id: &str,
    clip_in: i64,
    clip_out: i64,
    position: i64,
) -> Result<usize, ModelError> {
    if find_producer(root, producer_id).is_none() {
        return Err(ModelError::NotFound(format!("produtor não encontrado: {producer_id}")));
    }
    if clip_out < clip_in {
        return Err(ModelError::Invalid(format!(
            "out ({clip_out}) não pode ser menor que in ({clip_in})"
        )));
    }

    let playlist = find_playlist_mut(root, track_id)
        .ok_or_else(|| ModelError::NotFound(format!("trilha não encontrada: {track_id}")))?;
    let current_len = track_length(playlist);

    if position > current_len {
        let mut blank = Element::new("blank");
        blank.attributes.insert("length".to_string(), (position - current_len).to_string());
        playlist.children.push(XMLNode::Element(blank));
    } else if position < current_len {
        return Err(ModelError::Invalid(format!(
            "posição {position} sobrepõe clipes existentes nesta trilha (fim atual: {current_len}); \
             esta fase não faz inserção com deslocamento (\"ripple\")"
        )));
    }

    let mut entry = Element::new("entry");
    entry.attributes.insert("producer".to_string(), producer_id.to_string());
    entry.attributes.insert("in".to_string(), clip_in.to_string());
    entry.attributes.insert("out".to_string(), clip_out.to_string());
    playlist.children.push(XMLNode::Element(entry));

    let index = playlist
        .children
        .iter()
        .filter(|n| matches!(n, XMLNode::Element(e) if e.name == "entry" || e.name == "blank"))
        .count()
        - 1;
    Ok(index)
}

/// Remove o clipe no índice `index` da trilha `track_id` (vira um `<blank>`
/// do mesmo tamanho, para não deslocar os clipes seguintes).
pub fn remove_clip_from_track(root: &mut Element, track_id: &str, index: usize) -> Result<(), ModelError> {
    let playlist = find_playlist_mut(root, track_id)
        .ok_or_else(|| ModelError::NotFound(format!("trilha não encontrada: {track_id}")))?;
    let slot_index = nth_slot_index(playlist, index)
        .ok_or_else(|| ModelError::NotFound(format!("clipe não encontrado: {track_id}:{index}")))?;
    let length = entry_length(playlist.children[slot_index].as_element().expect("é um Element"));
    let mut blank = Element::new("blank");
    blank.attributes.insert("length".to_string(), length.to_string());
    playlist.children[slot_index] = XMLNode::Element(blank);
    Ok(())
}

/// Índice, dentro de `playlist.children`, do n-ésimo nó `entry`/`blank`
/// (0-based) — a referência de clipe externa é esse n, não o índice bruto
/// em `children` (que pode incluir outros tipos de nó).
fn nth_slot_index(playlist: &Element, n: usize) -> Option<usize> {
    playlist
        .children
        .iter()
        .enumerate()
        .filter(|(_, node)| matches!(node, XMLNode::Element(e) if e.name == "entry" || e.name == "blank"))
        .nth(n)
        .map(|(i, _)| i)
}

/// Lê o clipe no índice `index` da trilha `track_id` (ignora `<blank>`;
/// devolve `None` se esse slot for um `<blank>` ou não existir).
pub fn get_clip<'a>(root: &'a Element, track_id: &str, index: usize) -> Option<&'a Element> {
    let playlist = find_playlist(root, track_id)?;
    let slot_index = nth_slot_index(playlist, index)?;
    match &playlist.children[slot_index] {
        XMLNode::Element(e) if e.name == "entry" => Some(e),
        _ => None,
    }
}

pub fn get_clip_mut<'a>(root: &'a mut Element, track_id: &str, index: usize) -> Option<&'a mut Element> {
    let playlist = find_playlist_mut(root, track_id)?;
    let slot_index = nth_slot_index(playlist, index)?;
    match playlist.children.get_mut(slot_index) {
        Some(XMLNode::Element(e)) if e.name == "entry" => Some(e),
        _ => None,
    }
}

/// Lista `(índice, producer_id, in, out)` dos clipes (não `<blank>`) de uma trilha.
pub fn list_clips(root: &Element, track_id: &str) -> Vec<(usize, String, i64, i64)> {
    let Some(playlist) = find_playlist(root, track_id) else {
        return Vec::new();
    };
    playlist
        .children
        .iter()
        .filter_map(XMLNode::as_element)
        .filter(|e| e.name == "entry" || e.name == "blank")
        .enumerate()
        .filter_map(|(i, e)| {
            if e.name != "entry" {
                return None;
            }
            let producer = e.attributes.get("producer")?.clone();
            let in_f = e.attributes.get("in").and_then(|s| s.parse().ok()).unwrap_or(0);
            let out_f = e.attributes.get("out").and_then(|s| s.parse().ok()).unwrap_or(0);
            Some((i, producer, in_f, out_f))
        })
        .collect()
}

/// Duração total do projeto: o maior fim de trilha entre todas as trilhas.
pub fn project_duration(root: &Element) -> i64 {
    track_ids(root)
        .iter()
        .filter_map(|id| find_playlist(root, id))
        .map(track_length)
        .max()
        .unwrap_or(0)
}

// ---------------------------------------------------------------------------
// Velocidade e reversão de clipe (produtor `timewarp`, confirmado em
// `melt -query producer` neste ambiente)
// ---------------------------------------------------------------------------

/// Define a velocidade de reprodução de um clipe (1.0 normal, 2.0 dobro,
/// 0.5 metade, negativo = reverso), envolvendo o produtor original num
/// produtor `timewarp:<velocidade>:<resource original>`. O produtor
/// original (e a bin) não são alterados; o `<entry>` da trilha passa a
/// apontar para o novo produtor. Chamar de novo sobre o mesmo clipe
/// reaplica a partir do recurso original (guardado como propriedade no
/// produtor-wrapper), em vez de encadear vários `timewarp`.
pub fn set_clip_speed(
    root: &mut Element,
    track_id: &str,
    index: usize,
    velocidade: f64,
) -> Result<(), ModelError> {
    if velocidade == 0.0 {
        return Err(ModelError::Invalid("a velocidade não pode ser zero".to_string()));
    }

    let producer_id = get_clip(root, track_id, index)
        .ok_or_else(|| ModelError::NotFound(format!("clipe não encontrado: {track_id}:{index}")))?
        .attributes
        .get("producer")
        .cloned()
        .ok_or_else(|| ModelError::Invalid("entrada sem produtor".to_string()))?;

    let original = find_producer(root, &producer_id)
        .ok_or_else(|| ModelError::NotFound(format!("produtor não encontrado: {producer_id}")))?;

    // Se o produtor já é um wrapper de velocidade de uma chamada anterior,
    // parte do recurso/comprimento *originais* guardados nele, em vez de
    // encadear "timewarp:2:timewarp:2:arquivo.mp4".
    let recurso_original = get_property(original, "kdenlive-mcp-plus:original_resource")
        .or_else(|| get_property(original, "resource"))
        .ok_or_else(|| ModelError::Invalid("produtor sem propriedade \"resource\"".to_string()))?;
    let comprimento_original: i64 = get_property(original, "kdenlive-mcp-plus:original_length")
        .and_then(|v| v.parse().ok())
        .or_else(|| {
            let i: i64 = original.attributes.get("in")?.parse().ok()?;
            let o: i64 = original.attributes.get("out")?.parse().ok()?;
            Some(o - i + 1)
        })
        .unwrap_or(1);
    let nome = get_property(original, "kdenlive:clipname").unwrap_or_else(|| producer_id.clone());

    let novo_id = next_available_n(root, "producer", "producer");
    let novo_id = format!("producer{novo_id}");
    let mut wrapper = Element::new("producer");
    wrapper.attributes.insert("id".to_string(), novo_id.clone());
    let novo_comprimento = ((comprimento_original as f64) / velocidade.abs()).round().max(1.0) as i64;
    wrapper.attributes.insert("in".to_string(), "0".to_string());
    wrapper.attributes.insert("out".to_string(), (novo_comprimento - 1).to_string());
    set_property(&mut wrapper, "mlt_service", "timewarp");
    set_property(&mut wrapper, "resource", &format!("{velocidade}:{recurso_original}"));
    set_property(&mut wrapper, "kdenlive:clipname", &nome);
    set_property(&mut wrapper, "kdenlive-mcp-plus:original_resource", &recurso_original);
    set_property(&mut wrapper, "kdenlive-mcp-plus:original_length", &comprimento_original.to_string());
    set_property(&mut wrapper, "kdenlive-mcp-plus:speed", &velocidade.to_string());

    let insert_at = root
        .children
        .iter()
        .position(|n| matches!(n, XMLNode::Element(e) if attr_eq(e, "playlist", "id", MAIN_BIN_ID)))
        .unwrap_or(root.children.len());
    root.children.insert(insert_at, XMLNode::Element(wrapper));

    let entry = get_clip_mut(root, track_id, index).expect("já confirmado acima que existe");
    entry.attributes.insert("producer".to_string(), novo_id);
    entry.attributes.insert("in".to_string(), "0".to_string());
    entry.attributes.insert("out".to_string(), (novo_comprimento - 1).to_string());

    Ok(())
}

/// Velocidade atual de um clipe (1.0 se ele nunca passou por
/// `set_clip_speed`).
pub fn clip_speed(root: &Element, track_id: &str, index: usize) -> f64 {
    get_clip(root, track_id, index)
        .and_then(|entry| entry.attributes.get("producer"))
        .and_then(|id| find_producer(root, id))
        .and_then(|p| get_property(p, "kdenlive-mcp-plus:speed"))
        .and_then(|v| v.parse().ok())
        .unwrap_or(1.0)
}

/// Inverte a direção de reprodução do clipe (alterna o sinal da
/// velocidade atual).
pub fn reverse_clip(root: &mut Element, track_id: &str, index: usize) -> Result<(), ModelError> {
    let atual = clip_speed(root, track_id, index);
    set_clip_speed(root, track_id, index, -atual)
}

// ---------------------------------------------------------------------------
// Filtros (anexados ao `<entry>` do clipe — confirmado empiricamente que o
// MLT aceita `<filter>` como filho de `<entry>`, dando um efeito por corte,
// independente de outros cortes do mesmo produtor)
// ---------------------------------------------------------------------------

fn next_child_id(parent: &Element, tag: &str, prefix: &str) -> String {
    let mut max_n: i64 = -1;
    for child in parent.children.iter().filter_map(XMLNode::as_element) {
        if child.name == tag
            && let Some(id) = child.attributes.get("id")
            && let Some(num) = id.strip_prefix(prefix).and_then(|s| s.parse::<i64>().ok())
        {
            max_n = max_n.max(num);
        }
    }
    format!("{prefix}{}", max_n + 1)
}

/// Anexa um filtro (efeito) ao clipe `track_id:index`. Devolve o id do
/// filtro (único dentro deste clipe, não global).
pub fn add_filter(root: &mut Element, track_id: &str, index: usize, service: &str) -> Result<String, ModelError> {
    let entry = get_clip_mut(root, track_id, index)
        .ok_or_else(|| ModelError::NotFound(format!("clipe não encontrado: {track_id}:{index}")))?;
    let filter_id = next_child_id(entry, "filter", "filter");
    let mut filtro = Element::new("filter");
    filtro.attributes.insert("id".to_string(), filter_id.clone());
    filtro.attributes.insert("mlt_service".to_string(), service.to_string());
    entry.children.push(XMLNode::Element(filtro));
    Ok(filter_id)
}

pub fn remove_filter(root: &mut Element, track_id: &str, index: usize, filter_id: &str) -> Result<(), ModelError> {
    let entry = get_clip_mut(root, track_id, index)
        .ok_or_else(|| ModelError::NotFound(format!("clipe não encontrado: {track_id}:{index}")))?;
    let before = entry.children.len();
    entry.children.retain(|n| !matches!(n, XMLNode::Element(e) if attr_eq(e, "filter", "id", filter_id)));
    if entry.children.len() == before {
        return Err(ModelError::NotFound(format!("filtro não encontrado: {filter_id}")));
    }
    Ok(())
}

/// `(id, mlt_service)` de cada filtro anexado ao clipe.
pub fn list_filters(root: &Element, track_id: &str, index: usize) -> Vec<(String, String)> {
    let Some(entry) = get_clip(root, track_id, index) else {
        return Vec::new();
    };
    entry
        .children
        .iter()
        .filter_map(XMLNode::as_element)
        .filter(|e| e.name == "filter")
        .filter_map(|e| Some((e.attributes.get("id")?.clone(), e.attributes.get("mlt_service")?.clone())))
        .collect()
}

pub fn set_filter_param(
    root: &mut Element,
    track_id: &str,
    index: usize,
    filter_id: &str,
    param: &str,
    value: &str,
) -> Result<(), ModelError> {
    let entry = get_clip_mut(root, track_id, index)
        .ok_or_else(|| ModelError::NotFound(format!("clipe não encontrado: {track_id}:{index}")))?;
    let filtro = entry
        .children
        .iter_mut()
        .filter_map(XMLNode::as_mut_element)
        .find(|e| attr_eq(e, "filter", "id", filter_id))
        .ok_or_else(|| ModelError::NotFound(format!("filtro não encontrado: {filter_id}")))?;
    set_property(filtro, param, value);
    Ok(())
}

/// Encontra (ou cria, se nenhum existir ainda) o primeiro filtro do serviço
/// `service` no clipe, e define `param=value` nele. Usado por ferramentas
/// de conveniência como `clip_set_opacity`, que não querem expor o id do
/// filtro ao agente.
pub fn set_or_create_filter_param(
    root: &mut Element,
    track_id: &str,
    index: usize,
    service: &str,
    param: &str,
    value: &str,
) -> Result<String, ModelError> {
    let existente = list_filters(root, track_id, index)
        .into_iter()
        .find(|(_, s)| s == service)
        .map(|(id, _)| id);
    let filter_id = match existente {
        Some(id) => id,
        None => add_filter(root, track_id, index, service)?,
    };
    set_filter_param(root, track_id, index, &filter_id, param, value)?;
    Ok(filter_id)
}

// ---------------------------------------------------------------------------
// Transições (entre duas trilhas, como `<transition>` filho do tractor
// principal — sintaxe confirmada empiricamente com `melt`)
// ---------------------------------------------------------------------------

fn track_index(root: &Element, track_id: &str) -> Option<usize> {
    track_ids(root).iter().position(|id| id == track_id)
}

/// Cria uma transição `service` (ex. `"luma"` para dissolve/wipe) entre as
/// trilhas `track_a` e `track_b`. Devolve o id da transição.
pub fn add_transition(
    root: &mut Element,
    service: &str,
    track_a: &str,
    track_b: &str,
) -> Result<String, ModelError> {
    let a = track_index(root, track_a).ok_or_else(|| ModelError::NotFound(format!("trilha não encontrada: {track_a}")))?;
    let b = track_index(root, track_b).ok_or_else(|| ModelError::NotFound(format!("trilha não encontrada: {track_b}")))?;

    let tractor = main_tractor_mut(root)
        .ok_or_else(|| ModelError::NotFound("tractor principal não encontrado".to_string()))?;
    let transition_id = next_child_id(tractor, "transition", "transition");
    let mut transicao = Element::new("transition");
    transicao.attributes.insert("id".to_string(), transition_id.clone());
    transicao.attributes.insert("mlt_service".to_string(), service.to_string());
    set_property(&mut transicao, "a_track", &a.to_string());
    set_property(&mut transicao, "b_track", &b.to_string());
    tractor.children.push(XMLNode::Element(transicao));
    Ok(transition_id)
}

pub fn remove_transition(root: &mut Element, transition_id: &str) -> Result<(), ModelError> {
    let tractor = main_tractor_mut(root)
        .ok_or_else(|| ModelError::NotFound("tractor principal não encontrado".to_string()))?;
    let before = tractor.children.len();
    tractor
        .children
        .retain(|n| !matches!(n, XMLNode::Element(e) if attr_eq(e, "transition", "id", transition_id)));
    if tractor.children.len() == before {
        return Err(ModelError::NotFound(format!("transição não encontrada: {transition_id}")));
    }
    Ok(())
}

pub fn list_transitions(root: &Element) -> Vec<(String, String)> {
    let Some(tractor) = main_tractor(root) else {
        return Vec::new();
    };
    tractor
        .children
        .iter()
        .filter_map(XMLNode::as_element)
        .filter(|e| e.name == "transition")
        .filter_map(|e| Some((e.attributes.get("id")?.clone(), e.attributes.get("mlt_service")?.clone())))
        .collect()
}

pub fn set_transition_param(
    root: &mut Element,
    transition_id: &str,
    param: &str,
    value: &str,
) -> Result<(), ModelError> {
    let tractor = main_tractor_mut(root)
        .ok_or_else(|| ModelError::NotFound("tractor principal não encontrado".to_string()))?;
    let transicao = tractor
        .children
        .iter_mut()
        .filter_map(XMLNode::as_mut_element)
        .find(|e| attr_eq(e, "transition", "id", transition_id))
        .ok_or_else(|| ModelError::NotFound(format!("transição não encontrada: {transition_id}")))?;
    set_property(transicao, param, value);
    Ok(())
}

// ---------------------------------------------------------------------------
// Guias e marcadores: convenção própria deste projeto (não confirmada como
// igual ao que o Kdenlive real grava — ver docs/09-riscos-e-bugs.md),
// guardada como JSON numa propriedade do tractor principal.
// ---------------------------------------------------------------------------

const PROP_GUIDES: &str = "kdenlive-mcp-plus:guides";
const PROP_MARKERS: &str = "kdenlive-mcp-plus:markers";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Guide {
    pub id: String,
    pub position: i64,
    pub comment: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Marker {
    pub id: String,
    pub position: i64,
    pub comment: String,
    /// Referência de clipe (`"<trilha>:<índice>"`), se este for um
    /// marcador de clipe em vez de um marcador geral da timeline.
    pub clip_ref: Option<String>,
}

fn ler_lista<T: for<'de> Deserialize<'de>>(root: &Element, prop: &str) -> Vec<T> {
    main_tractor(root)
        .and_then(|t| get_property(t, prop))
        .and_then(|texto| serde_json::from_str(&texto).ok())
        .unwrap_or_default()
}

fn escrever_lista<T: Serialize>(root: &mut Element, prop: &str, lista: &[T]) -> Result<(), ModelError> {
    let texto = serde_json::to_string(lista).map_err(|e| ModelError::Invalid(e.to_string()))?;
    let tractor = main_tractor_mut(root)
        .ok_or_else(|| ModelError::NotFound("tractor principal não encontrado".to_string()))?;
    set_property(tractor, prop, &texto);
    Ok(())
}

fn proximo_id_de_lista<T>(lista: &[T], prefixo: &str, id_de: impl Fn(&T) -> &str) -> String {
    let mut max_n: i64 = -1;
    for item in lista {
        if let Some(num) = id_de(item).strip_prefix(prefixo).and_then(|s| s.parse::<i64>().ok()) {
            max_n = max_n.max(num);
        }
    }
    format!("{prefixo}{}", max_n + 1)
}

pub fn list_guides(root: &Element) -> Vec<Guide> {
    ler_lista(root, PROP_GUIDES)
}

pub fn add_guide(root: &mut Element, position: i64, comment: &str) -> Result<String, ModelError> {
    let mut lista = list_guides(root);
    let id = proximo_id_de_lista(&lista, "guide", |g| &g.id);
    lista.push(Guide { id: id.clone(), position, comment: comment.to_string() });
    escrever_lista(root, PROP_GUIDES, &lista)?;
    Ok(id)
}

pub fn remove_guide(root: &mut Element, id: &str) -> Result<(), ModelError> {
    let mut lista = list_guides(root);
    let antes = lista.len();
    lista.retain(|g| g.id != id);
    if lista.len() == antes {
        return Err(ModelError::NotFound(format!("guia não encontrada: {id}")));
    }
    escrever_lista(root, PROP_GUIDES, &lista)
}

pub fn get_guide(root: &Element, id: &str) -> Option<Guide> {
    list_guides(root).into_iter().find(|g| g.id == id)
}

pub fn update_guide(
    root: &mut Element,
    id: &str,
    position: Option<i64>,
    comment: Option<&str>,
) -> Result<(), ModelError> {
    let mut lista = list_guides(root);
    let guia = lista
        .iter_mut()
        .find(|g| g.id == id)
        .ok_or_else(|| ModelError::NotFound(format!("guia não encontrada: {id}")))?;
    if let Some(p) = position {
        guia.position = p;
    }
    if let Some(c) = comment {
        guia.comment = c.to_string();
    }
    escrever_lista(root, PROP_GUIDES, &lista)
}

pub fn list_markers(root: &Element) -> Vec<Marker> {
    ler_lista(root, PROP_MARKERS)
}

pub fn add_marker(
    root: &mut Element,
    position: i64,
    comment: &str,
    clip_ref: Option<&str>,
) -> Result<String, ModelError> {
    let mut lista = list_markers(root);
    let id = proximo_id_de_lista(&lista, "marker", |m| &m.id);
    lista.push(Marker {
        id: id.clone(),
        position,
        comment: comment.to_string(),
        clip_ref: clip_ref.map(str::to_string),
    });
    escrever_lista(root, PROP_MARKERS, &lista)?;
    Ok(id)
}

pub fn remove_marker(root: &mut Element, id: &str) -> Result<(), ModelError> {
    let mut lista = list_markers(root);
    let antes = lista.len();
    lista.retain(|m| m.id != id);
    if lista.len() == antes {
        return Err(ModelError::NotFound(format!("marcador não encontrado: {id}")));
    }
    escrever_lista(root, PROP_MARKERS, &lista)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn documento_novo_tem_perfil_bin_e_tractor() {
        let doc = new_document(crate::model::profile::DEFAULT_PROFILE).expect("perfil padrão válido");
        assert!(profile_element(&doc).is_some());
        assert!(main_bin(&doc).is_some());
        assert!(main_tractor(&doc).is_some());
        assert!(track_ids(&doc).is_empty());
    }

    #[test]
    fn perfil_desconhecido_gera_erro() {
        let erro = new_document("isto-nao-existe").expect_err("deveria falhar");
        assert!(matches!(erro, ModelError::Invalid(_)));
    }

    #[test]
    fn importar_clipe_entra_na_bin_com_o_id_dado() {
        let mut doc = new_document(DEFAULT_PROFILE_PARA_TESTE).expect("perfil válido");
        let id1 = add_bin_clip(&mut doc, "producer0", "/tmp/a.mp4", "a.mp4", 0, 99).expect("deveria importar");
        let id2 = add_bin_clip(&mut doc, "producer1", "/tmp/b.mp4", "b.mp4", 0, 49).expect("deveria importar");
        assert_ne!(id1, id2);
        assert_eq!(bin_clip_ids(&doc), vec![id1.clone(), id2.clone()]);
        assert_eq!(get_property(find_producer(&doc, &id1).unwrap(), "kdenlive:clipname"), Some("a.mp4".to_string()));
    }

    #[test]
    fn adicionar_trilha_e_clipe_depois_remover() {
        let mut doc = new_document(DEFAULT_PROFILE_PARA_TESTE).expect("perfil válido");
        let producer_id = add_bin_clip(&mut doc, "producer0", "/tmp/a.mp4", "a.mp4", 0, 99).expect("importa");
        let track_id = add_track(&mut doc, "playlist0", TrackKind::Video).expect("cria trilha");

        let index = add_clip_to_track(&mut doc, &track_id, &producer_id, 0, 49, 0).expect("posiciona clipe");
        assert_eq!(index, 0);
        assert_eq!(list_clips(&doc, &track_id), vec![(0, producer_id.clone(), 0, 49)]);
        assert_eq!(track_length(find_playlist(&doc, &track_id).unwrap()), 50);

        // sobrepor deve falhar
        let erro = add_clip_to_track(&mut doc, &track_id, &producer_id, 0, 9, 10).expect_err("deveria falhar: sobrepõe");
        assert!(matches!(erro, ModelError::Invalid(_)));

        // numa posição depois do fim, cria um blank de preenchimento antes do
        // clipe novo; os slots passam a ser [entry0, blank1, entry2]
        let index2 = add_clip_to_track(&mut doc, &track_id, &producer_id, 0, 9, 100).expect("posiciona com gap");
        assert_eq!(index2, 2);
        assert_eq!(project_duration(&doc), 110);

        // remove o primeiro clipe (slot 0): ele vira um blank, não desaparece
        remove_clip_from_track(&mut doc, &track_id, 0).expect("remove o primeiro clipe");
        assert_eq!(list_clips(&doc, &track_id), vec![(2, producer_id.clone(), 0, 9)]);
        assert_eq!(project_duration(&doc), 110, "remover um clipe não deve encurtar a trilha");

        remove_track(&mut doc, &track_id).expect("remove a trilha");
        assert!(track_ids(&doc).is_empty());
        // o produtor continua existindo (ainda está na bin)
        assert!(find_producer(&doc, &producer_id).is_some());
    }

    #[test]
    fn remover_clipe_da_bin_tambem_remove_o_produtor_quando_nao_usado_em_trilha() {
        let mut doc = new_document(DEFAULT_PROFILE_PARA_TESTE).expect("perfil válido");
        let producer_id = add_bin_clip(&mut doc, "producer0", "/tmp/a.mp4", "a.mp4", 0, 99).expect("importa");
        remove_bin_clip(&mut doc, &producer_id).expect("remove da bin");
        assert!(bin_clip_ids(&doc).is_empty());
        assert!(find_producer(&doc, &producer_id).is_none());
    }

    #[test]
    fn remover_clipe_da_bin_preserva_produtor_ainda_usado_numa_trilha() {
        let mut doc = new_document(DEFAULT_PROFILE_PARA_TESTE).expect("perfil válido");
        let producer_id = add_bin_clip(&mut doc, "producer0", "/tmp/a.mp4", "a.mp4", 0, 99).expect("importa");
        let track_id = add_track(&mut doc, "playlist0", TrackKind::Video).expect("cria trilha");
        add_clip_to_track(&mut doc, &track_id, &producer_id, 0, 49, 0).expect("posiciona");

        remove_bin_clip(&mut doc, &producer_id).expect("remove da bin");
        assert!(bin_clip_ids(&doc).is_empty());
        assert!(
            find_producer(&doc, &producer_id).is_some(),
            "o produtor ainda está em uso numa trilha, não deveria ser apagado"
        );
    }

    #[test]
    fn next_available_n_inicializa_a_partir_do_documento() {
        let mut doc = new_document(DEFAULT_PROFILE_PARA_TESTE).expect("perfil válido");
        assert_eq!(next_available_n(&doc, "producer", "producer"), 0);
        add_bin_clip(&mut doc, "producer0", "/tmp/a.mp4", "a", 0, 9).expect("importa");
        add_bin_clip(&mut doc, "producer2", "/tmp/b.mp4", "b", 0, 9).expect("importa (id não sequencial de propósito)");
        assert_eq!(next_available_n(&doc, "producer", "producer"), 3);
    }

    #[test]
    fn hidden_e_muted_sao_independentes_um_do_outro() {
        let mut doc = new_document(DEFAULT_PROFILE_PARA_TESTE).expect("perfil válido");
        let track_id = add_track(&mut doc, "playlist0", TrackKind::Video).expect("cria trilha");

        assert_eq!(track_hidden_muted(&doc, &track_id), Some((false, false)));

        track_set_hidden(&mut doc, &track_id, true).expect("oculta");
        assert_eq!(track_hidden_muted(&doc, &track_id), Some((true, false)));
        assert_eq!(track_kind(&doc, &track_id), Some(TrackKind::Video), "ocultar não deve mudar o tipo da trilha");

        track_set_muted(&mut doc, &track_id, true).expect("muta");
        assert_eq!(track_hidden_muted(&doc, &track_id), Some((true, true)));

        track_set_hidden(&mut doc, &track_id, false).expect("desoculta");
        assert_eq!(track_hidden_muted(&doc, &track_id), Some((false, true)), "deveria continuar mudo");

        track_set_muted(&mut doc, &track_id, false).expect("desmuta");
        assert_eq!(track_hidden_muted(&doc, &track_id), Some((false, false)));
    }

    #[test]
    fn trilha_de_audio_comeca_com_video_oculto_mas_pode_ser_desmutada() {
        let mut doc = new_document(DEFAULT_PROFILE_PARA_TESTE).expect("perfil válido");
        let track_id = add_track(&mut doc, "playlist0", TrackKind::Audio).expect("cria trilha");
        assert_eq!(track_kind(&doc, &track_id), Some(TrackKind::Audio));
        assert_eq!(track_hidden_muted(&doc, &track_id), Some((true, false)));

        track_set_muted(&mut doc, &track_id, true).expect("muta");
        assert_eq!(track_hidden_muted(&doc, &track_id), Some((true, true)));
        assert_eq!(track_kind(&doc, &track_id), Some(TrackKind::Audio), "mutar não deve mudar o tipo da trilha");
    }

    #[test]
    fn nome_e_trava_da_trilha_sao_lidos_de_volta() {
        let mut doc = new_document(DEFAULT_PROFILE_PARA_TESTE).expect("perfil válido");
        let track_id = add_track(&mut doc, "playlist0", TrackKind::Video).expect("cria trilha");

        assert_eq!(track_name(&doc, &track_id), None);
        assert!(!track_is_locked(&doc, &track_id));

        track_set_name(&mut doc, &track_id, "Trilha principal").expect("renomeia");
        track_set_locked(&mut doc, &track_id, true).expect("trava");

        assert_eq!(track_name(&doc, &track_id), Some("Trilha principal".to_string()));
        assert!(track_is_locked(&doc, &track_id));
    }

    #[test]
    fn mover_trilha_troca_a_ordem_no_tractor() {
        let mut doc = new_document(DEFAULT_PROFILE_PARA_TESTE).expect("perfil válido");
        let t0 = add_track(&mut doc, "playlist0", TrackKind::Video).expect("cria trilha 0");
        let t1 = add_track(&mut doc, "playlist1", TrackKind::Video).expect("cria trilha 1");
        let t2 = add_track(&mut doc, "playlist2", TrackKind::Video).expect("cria trilha 2");
        assert_eq!(track_ids(&doc), vec![t0.clone(), t1.clone(), t2.clone()]);

        track_move_up(&mut doc, &t1).expect("move a do meio para cima");
        assert_eq!(track_ids(&doc), vec![t1.clone(), t0.clone(), t2.clone()]);

        track_move_down(&mut doc, &t1).expect("move de volta para baixo");
        assert_eq!(track_ids(&doc), vec![t0.clone(), t1.clone(), t2.clone()]);

        let erro = track_move_up(&mut doc, &t0).expect_err("já é a primeira");
        assert!(matches!(erro, ModelError::Invalid(_)));
        let erro = track_move_down(&mut doc, &t2).expect_err("já é a última");
        assert!(matches!(erro, ModelError::Invalid(_)));
    }

    #[test]
    fn set_clip_speed_envolve_em_timewarp_e_encurta_o_clipe() {
        let mut doc = new_document(DEFAULT_PROFILE_PARA_TESTE).expect("perfil válido");
        let producer_id = add_bin_clip(&mut doc, "producer0", "/tmp/a.mp4", "a.mp4", 0, 99).expect("importa");
        let track_id = add_track(&mut doc, "playlist0", TrackKind::Video).expect("cria trilha");
        add_clip_to_track(&mut doc, &track_id, &producer_id, 0, 99, 0).expect("posiciona (100 quadros)");

        assert_eq!(clip_speed(&doc, &track_id, 0), 1.0);

        set_clip_speed(&mut doc, &track_id, 0, 2.0).expect("dobra a velocidade");
        assert_eq!(clip_speed(&doc, &track_id, 0), 2.0);
        let (_, novo_producer, in_f, out_f) = list_clips(&doc, &track_id)[0].clone();
        assert_ne!(novo_producer, producer_id, "deveria apontar para um novo produtor wrapper");
        assert_eq!(out_f - in_f + 1, 50, "100 quadros a 2x deveriam caber em 50");

        let wrapper = find_producer(&doc, &novo_producer).expect("produtor wrapper deve existir");
        assert_eq!(get_property(wrapper, "mlt_service"), Some("timewarp".to_string()));
        assert_eq!(get_property(wrapper, "resource"), Some("2:/tmp/a.mp4".to_string()));

        // chamar de novo não deve encadear "timewarp:timewarp:..."
        set_clip_speed(&mut doc, &track_id, 0, 4.0).expect("quadruplica a partir do original");
        let (_, producer_4x, in4, out4) = list_clips(&doc, &track_id)[0].clone();
        let wrapper_4x = find_producer(&doc, &producer_4x).expect("produtor wrapper 4x");
        assert_eq!(get_property(wrapper_4x, "resource"), Some("4:/tmp/a.mp4".to_string()));
        assert_eq!(out4 - in4 + 1, 25, "100 quadros a 4x deveriam caber em 25");

        // o produtor original da bin não foi alterado
        assert_eq!(find_producer(&doc, &producer_id).unwrap().attributes.get("out"), Some(&"99".to_string()));
    }

    #[test]
    fn reverse_clip_inverte_o_sinal_da_velocidade_atual() {
        let mut doc = new_document(DEFAULT_PROFILE_PARA_TESTE).expect("perfil válido");
        let producer_id = add_bin_clip(&mut doc, "producer0", "/tmp/a.mp4", "a.mp4", 0, 49).expect("importa");
        let track_id = add_track(&mut doc, "playlist0", TrackKind::Video).expect("cria trilha");
        add_clip_to_track(&mut doc, &track_id, &producer_id, 0, 49, 0).expect("posiciona");

        reverse_clip(&mut doc, &track_id, 0).expect("inverte");
        assert_eq!(clip_speed(&doc, &track_id, 0), -1.0);

        reverse_clip(&mut doc, &track_id, 0).expect("inverte de volta");
        assert_eq!(clip_speed(&doc, &track_id, 0), 1.0);
    }

    #[test]
    fn filtros_sao_anexados_ao_clipe_nao_ao_produtor_compartilhado() {
        let mut doc = new_document(DEFAULT_PROFILE_PARA_TESTE).expect("perfil válido");
        let producer_id = add_bin_clip(&mut doc, "producer0", "/tmp/a.mp4", "a.mp4", 0, 49).expect("importa");
        let track_id = add_track(&mut doc, "playlist0", TrackKind::Video).expect("cria trilha");
        add_clip_to_track(&mut doc, &track_id, &producer_id, 0, 24, 0).expect("primeiro clipe");
        add_clip_to_track(&mut doc, &track_id, &producer_id, 0, 24, 25).expect("segundo clipe (mesmo produtor)");

        assert!(list_filters(&doc, &track_id, 0).is_empty());
        let filter_id = add_filter(&mut doc, &track_id, 0, "brightness").expect("adiciona filtro no primeiro");
        assert_eq!(list_filters(&doc, &track_id, 0), vec![(filter_id.clone(), "brightness".to_string())]);
        assert!(
            list_filters(&doc, &track_id, 1).is_empty(),
            "o segundo clipe usa o mesmo produtor, mas não deveria herdar o filtro do primeiro"
        );

        set_filter_param(&mut doc, &track_id, 0, &filter_id, "alpha", "0.5").expect("ajusta parâmetro");

        remove_filter(&mut doc, &track_id, 0, &filter_id).expect("remove");
        assert!(list_filters(&doc, &track_id, 0).is_empty());
    }

    #[test]
    fn transicoes_ficam_no_tractor_entre_duas_trilhas() {
        let mut doc = new_document(DEFAULT_PROFILE_PARA_TESTE).expect("perfil válido");
        let t0 = add_track(&mut doc, "playlist0", TrackKind::Video).expect("trilha 0");
        let t1 = add_track(&mut doc, "playlist1", TrackKind::Video).expect("trilha 1");

        assert!(list_transitions(&doc).is_empty());
        let transition_id = add_transition(&mut doc, "luma", &t0, &t1).expect("cria transição");
        assert_eq!(list_transitions(&doc), vec![(transition_id.clone(), "luma".to_string())]);

        set_transition_param(&mut doc, &transition_id, "softness", "0.5").expect("ajusta parâmetro");

        remove_transition(&mut doc, &transition_id).expect("remove");
        assert!(list_transitions(&doc).is_empty());
    }

    #[test]
    fn guias_sao_adicionadas_lidas_atualizadas_e_removidas() {
        let mut doc = new_document(DEFAULT_PROFILE_PARA_TESTE).expect("perfil válido");
        assert!(list_guides(&doc).is_empty());

        let id0 = add_guide(&mut doc, 100, "início da cena 2").expect("adiciona guia");
        let id1 = add_guide(&mut doc, 200, "corte").expect("adiciona outra guia");
        assert_eq!(list_guides(&doc).len(), 2);

        update_guide(&mut doc, &id0, Some(150), None).expect("atualiza posição");
        assert_eq!(get_guide(&doc, &id0).unwrap().position, 150);
        assert_eq!(get_guide(&doc, &id0).unwrap().comment, "início da cena 2", "não deveria mudar o comentário");

        remove_guide(&mut doc, &id1).expect("remove");
        assert_eq!(list_guides(&doc).len(), 1);
        assert!(get_guide(&doc, &id1).is_none());

        let erro = remove_guide(&mut doc, "guide-que-nao-existe").expect_err("deveria falhar");
        assert!(matches!(erro, ModelError::NotFound(_)));
    }

    #[test]
    fn marcadores_de_timeline_e_de_clipe_coexistem() {
        let mut doc = new_document(DEFAULT_PROFILE_PARA_TESTE).expect("perfil válido");
        let id_timeline = add_marker(&mut doc, 50, "marcador geral", None).expect("adiciona marcador de timeline");
        let id_clipe = add_marker(&mut doc, 10, "marcador do clipe", Some("playlist0:0")).expect("adiciona marcador de clipe");

        let lista = list_markers(&doc);
        assert_eq!(lista.len(), 2);
        assert!(lista.iter().any(|m| m.id == id_timeline && m.clip_ref.is_none()));
        assert!(lista.iter().any(|m| m.id == id_clipe && m.clip_ref.as_deref() == Some("playlist0:0")));

        remove_marker(&mut doc, &id_timeline).expect("remove");
        assert_eq!(list_markers(&doc).len(), 1);
    }

    #[test]
    fn documento_sobrevive_a_um_ciclo_completo_de_escrita_e_leitura() {
        let mut doc = new_document(DEFAULT_PROFILE_PARA_TESTE).expect("perfil válido");
        let producer_id = add_bin_clip(&mut doc, "producer0", "/tmp/a.mp4", "a.mp4", 0, 99).expect("importa");
        let track_id = add_track(&mut doc, "playlist0", TrackKind::Audio).expect("cria trilha");
        add_clip_to_track(&mut doc, &track_id, &producer_id, 0, 49, 0).expect("posiciona");

        let xml = to_string_pretty(&doc).expect("serializa");
        let reparsed = parse(&xml).expect("o XML gerado deve ser válido");

        assert_eq!(bin_clip_ids(&reparsed), vec![producer_id.clone()]);
        assert_eq!(track_ids(&reparsed), vec![track_id.clone()]);
        assert_eq!(track_kind(&reparsed, &track_id), Some(TrackKind::Audio));
        assert_eq!(list_clips(&reparsed, &track_id), list_clips(&doc, &track_id));
    }

    use profile::DEFAULT_PROFILE as DEFAULT_PROFILE_PARA_TESTE;
}
