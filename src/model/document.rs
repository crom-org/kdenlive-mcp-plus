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
    main_tractor(root)?
        .children
        .iter()
        .filter_map(XMLNode::as_element)
        .find(|e| attr_eq(e, "track", "producer", track_id))
        .map(|e| {
            if e.attributes.get("hide").map(String::as_str) == Some("video") {
                TrackKind::Audio
            } else {
                TrackKind::Video
            }
        })
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
