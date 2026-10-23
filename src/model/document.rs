//! Construção e consulta de um documento MLT XML em memória.
//!
//! Convenções deste projeto (documentadas aqui porque não são universais do
//! MLT nem garantidamente idênticas ao que o Kdenlive de verdade grava —
//! ver docs/09-riscos-e-bugs.md):
//!
//! - Formato do documento = o que o Kdenlive 23.08+ grava (confirmado lendo
//!   o código-fonte das versões 23.08.5 e 26.04.1 e projetos reais salvos
//!   pelo próprio Kdenlive — decisão D-040 em docs/decisoes.md), na ordem
//!   exigida pelo XML do MLT (só se referencia o que já foi definido):
//!   perfil → produtor preto `black_track` → produtores da bin → trilhas
//!   (cada uma um `<tractor>` com duas `<playlist>`) → **sequência** (um
//!   `<tractor>` cujo id é um UUID, com a `black_track` como trilha 0) →
//!   `main_bin` (com `xml_retain=1`, sem o qual o Kdenlive não acha a bin) →
//!   tractor do projeto (`kdenlive:projectTractor=1`, aponta para a
//!   sequência). Uma única sequência por documento.
//! - Cada clipe da bin tem `kdenlive:id` **numérico** e um
//!   `kdenlive:control_uuid` em formato UUID — é por eles que o Kdenlive
//!   liga um clipe da timeline ao clipe da bin ao abrir o projeto.
//! - Uma **referência de clipe**, nas ferramentas deste servidor, é a
//!   string `"<id da trilha>:<índice da entrada>"` (ex. `"track0:2"`).
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
/// Id do produtor de fundo preto que o Kdenlive exige como trilha 0 de toda
/// sequência (ele o reconhece por `kdenlive:playlistid=black_track`).
pub const BLACK_TRACK_ID: &str = "black_track";
pub const PROJECT_TRACTOR_ID: &str = "project_tractor";
/// Valor de `internal_added` que o Kdenlive grava nas transições internas de
/// composição de trilha (`mix`/`qtblend`) — ele as ignora ao abrir e as
/// recria; para o render via libmlt elas são o que mistura as trilhas.
const INTERNAL_ADDED: &str = "237";
const SEQUENCE_KDENLIVE_ID: &str = "1";
const SEQUENCE_FOLDER_ID: &str = "2";

/// UUID v4 aleatório no formato `{xxxxxxxx-xxxx-4xxx-yxxx-xxxxxxxxxxxx}`,
/// que o `QUuid` do Kdenlive aceita (um `kdenlive:control_uuid` que não
/// seja um UUID válido faz o Kdenlive 26 descartar o clipe da timeline).
pub fn new_uuid() -> String {
    use std::collections::hash_map::RandomState;
    use std::hash::{BuildHasher, Hasher};
    use std::sync::atomic::{AtomicU64, Ordering};
    static CONTADOR: AtomicU64 = AtomicU64::new(0);

    let n = CONTADOR.fetch_add(1, Ordering::Relaxed);
    let estado = RandomState::new();
    let mut bytes = [0u8; 16];
    for (metade, parte) in bytes.chunks_mut(8).enumerate() {
        let mut h = estado.build_hasher();
        h.write_u64(n);
        h.write_usize(metade);
        parte.copy_from_slice(&h.finish().to_le_bytes());
    }
    bytes[6] = (bytes[6] & 0x0f) | 0x40;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    let hex: String = bytes.iter().map(|b| format!("{b:02x}")).collect();
    format!("{{{}-{}-{}-{}-{}}}", &hex[0..8], &hex[8..12], &hex[12..16], &hex[16..20], &hex[20..32])
}

/// Identificador do documento no mesmo formato que o Kdenlive gera: os
/// milissegundos desde a época Unix, como texto decimal. O Kdenlive usa esse
/// número para nomear as pastas de cache do projeto, e **um arquivo sem ele
/// é tratado como "modificado" ao abrir** — o que faz o File > Revert (e o
/// recarregamento automático, D-042) pedir uma confirmação em vez de recarregar.
pub fn new_document_id() -> String {
    let ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    ms.to_string()
}

const PROP_DOCUMENT_ID: &str = "kdenlive:docproperties.documentid";

/// Garante que a bin tenha `documentid` (ver [`new_document_id`]). Não
/// mexe num id que já exista — ele vem do Kdenlive ou de uma sessão anterior.
pub fn ensure_document_id(root: &mut Element) {
    if let Some(bin) = main_bin_mut(root)
        && get_property(bin, PROP_DOCUMENT_ID).is_none()
    {
        set_property(bin, PROP_DOCUMENT_ID, &new_document_id());
    }
}

fn element_with_props(tag: &str, attrs: &[(&str, &str)], props: &[(&str, &str)]) -> Element {
    let mut el = Element::new(tag);
    for (k, v) in attrs {
        el.attributes.insert((*k).to_string(), (*v).to_string());
    }
    for (k, v) in props {
        set_property(&mut el, k, v);
    }
    el
}

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

/// Serializa o documento. Antes, numa cópia, ajusta o que depende da duração
/// atual da timeline (ver `sync_lengths`) — não muda o documento em memória.
pub fn to_string_pretty(root: &Element) -> Result<String, ModelError> {
    let mut copia = root.clone();
    sync_lengths(&mut copia);
    let mut buf = Vec::new();
    let config = EmitterConfig::new()
        .perform_indent(true)
        .write_document_declaration(true);
    copia
        .write_with_config(&mut buf, config)
        .map_err(|e| ModelError::Write(e.to_string()))?;
    String::from_utf8(buf).map_err(|e| ModelError::Write(e.to_string()))
}

/// Ajusta os `out` da black track, da sequência e do tractor do projeto para
/// a duração atual da timeline, e dá às transições do usuário sem intervalo
/// explícito o intervalo da timeline inteira. Necessário porque a black
/// track tem `length` "infinito" (como no Kdenlive real): sem limitar o
/// `out`, o render via libmlt nunca terminaria; e o Kdenlive só aceita
/// composição com posição e duração.
fn sync_lengths(root: &mut Element) {
    if !has_black_track(root) {
        return;
    }
    let out = (project_duration(root) - 1).max(0).to_string();
    let seq_id = sequence_tractor_id(root);
    for node in root.children.iter_mut() {
        let XMLNode::Element(e) = node else { continue };
        let id = e.attributes.get("id").cloned().unwrap_or_default();
        if e.name == "producer" && id == BLACK_TRACK_ID {
            e.attributes.insert("out".to_string(), out.clone());
        } else if e.name == "tractor" && Some(&id) == seq_id.as_ref() {
            e.attributes.insert("in".to_string(), "0".to_string());
            e.attributes.insert("out".to_string(), out.clone());
            for t in e.children.iter_mut().filter_map(XMLNode::as_mut_element) {
                if t.name == "transition"
                    && get_property(t, "internal_added").is_none()
                    && t.attributes.get("out").is_none()
                    && get_property(t, "out").is_none()
                {
                    t.attributes.insert("in".to_string(), "0".to_string());
                    t.attributes.insert("out".to_string(), out.clone());
                }
            }
        } else if e.name == "tractor" && get_property(e, "kdenlive:projectTractor").as_deref() == Some("1") {
            e.attributes.insert("in".to_string(), "0".to_string());
            e.attributes.insert("out".to_string(), out.clone());
            for t in e.children.iter_mut().filter_map(XMLNode::as_mut_element).filter(|t| t.name == "track") {
                t.attributes.insert("in".to_string(), "0".to_string());
                t.attributes.insert("out".to_string(), out.clone());
            }
        }
    }
}

/// Cria um documento novo, já no formato que o Kdenlive 23.08+ abre sem
/// correções (ver o comentário do módulo): perfil, black track, uma
/// sequência vazia, a bin (com a pasta de sequências do próprio Kdenlive) e o
/// tractor do projeto.
pub fn new_document(profile_name: &str) -> Result<Element, ModelError> {
    let profile = profile::find(profile_name).ok_or_else(|| {
        ModelError::Invalid(format!(
            "perfil desconhecido: \"{profile_name}\". Use project_list_profiles para ver os \
             perfis válidos."
        ))
    })?;
    let seq_owned = new_uuid();
    let seq = seq_owned.as_str();
    let pasta_seq = format!("kdenlive:folder.-1.{SEQUENCE_FOLDER_ID}");

    let mut root = Element::new("mlt");
    root.attributes.insert("LC_NUMERIC".to_string(), "C".to_string());
    root.attributes.insert("version".to_string(), "7.22.0".to_string());
    root.attributes.insert("producer".to_string(), MAIN_BIN_ID.to_string());

    let mut profile_el = Element::new("profile");
    for (k, v) in profile.to_xml_attrs() {
        profile_el.attributes.insert(k, v);
    }
    root.children.push(XMLNode::Element(profile_el));

    root.children.push(XMLNode::Element(element_with_props(
        "producer",
        &[("id", BLACK_TRACK_ID), ("in", "0"), ("out", "0")],
        &[
            ("length", "2147483647"),
            ("eof", "continue"),
            ("resource", "black"),
            ("aspect_ratio", "1"),
            ("mlt_service", "color"),
            ("kdenlive:playlistid", "black_track"),
            ("mlt_image_format", "rgba"),
            ("set.test_audio", "0"),
        ],
    )));

    let mut sequencia = element_with_props(
        "tractor",
        &[("id", seq), ("in", "0"), ("out", "0")],
        &[
            ("kdenlive:uuid", seq),
            ("kdenlive:clipname", "Sequência 1"),
            ("kdenlive:sequenceproperties.hasAudio", "1"),
            ("kdenlive:sequenceproperties.hasVideo", "1"),
            ("kdenlive:sequenceproperties.documentuuid", seq),
            ("kdenlive:control_uuid", seq),
            ("kdenlive:producer_type", "17"),
            ("kdenlive:id", SEQUENCE_KDENLIVE_ID),
            ("kdenlive:clip_type", "0"),
            ("kdenlive:folderid", SEQUENCE_FOLDER_ID),
        ],
    );
    sequencia
        .children
        .push(XMLNode::Element(element_with_props("track", &[("producer", BLACK_TRACK_ID)], &[])));
    root.children.push(XMLNode::Element(sequencia));

    let mut main_bin = element_with_props(
        "playlist",
        &[("id", MAIN_BIN_ID)],
        &[
            (pasta_seq.as_str(), "Sequências"),
            ("kdenlive:sequenceFolder", SEQUENCE_FOLDER_ID),
            // Sem a versão, o Kdenlive mostra "Arquivo de projeto incorreto"
            // (DOCUMENTVERSION em kdenlivedoc.cpp; D-037).
            ("kdenlive:docproperties.version", "1.1"),
            ("kdenlive:docproperties.uuid", seq),
            ("kdenlive:docproperties.opensequences", seq),
            ("kdenlive:docproperties.activetimeline", seq),
            ("kdenlive:docproperties.profile", profile_name),
            (PROP_DOCUMENT_ID, new_document_id().as_str()),
            // Sem isto o MLT não "retém" esta playlist e o Kdenlive abre
            // com a bin vazia — a causa de os clipes sumirem da timeline.
            ("xml_retain", "1"),
        ],
    );
    main_bin
        .children
        .push(XMLNode::Element(element_with_props("entry", &[("producer", seq), ("in", "0"), ("out", "0")], &[])));
    root.children.push(XMLNode::Element(main_bin));

    let mut projeto = element_with_props(
        "tractor",
        &[("id", PROJECT_TRACTOR_ID), ("in", "0"), ("out", "0")],
        &[("kdenlive:projectTractor", "1")],
    );
    projeto
        .children
        .push(XMLNode::Element(element_with_props("track", &[("producer", seq), ("in", "0"), ("out", "0")], &[])));
    root.children.push(XMLNode::Element(projeto));

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

/// Troca o perfil de vídeo do projeto por um perfil nomeado válido (ver
/// `model::profile`). Só altera o `<profile>` (e o nome do perfil gravado nas
/// propriedades do documento) — não recalcula nenhuma posição/duração já
/// gravada na timeline.
pub fn set_profile(root: &mut Element, profile_name: &str) -> Result<(), ModelError> {
    let definicao = profile::find(profile_name).ok_or_else(|| {
        ModelError::Invalid(format!(
            "perfil desconhecido: \"{profile_name}\". Use project_list_profiles para ver os \
             perfis válidos."
        ))
    })?;
    let elemento = profile_element_mut(root)
        .ok_or_else(|| ModelError::NotFound("elemento <profile> não encontrado".to_string()))?;
    elemento.attributes.clear();
    for (chave, valor) in definicao.to_xml_attrs() {
        elemento.attributes.insert(chave, valor);
    }
    if let Some(bin) = main_bin_mut(root)
        && get_property(bin, "kdenlive:docproperties.profile").is_some()
    {
        set_property(bin, "kdenlive:docproperties.profile", profile_name);
    }
    Ok(())
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

fn find_tractor<'a>(root: &'a Element, id: &str) -> Option<&'a Element> {
    root.children.iter().filter_map(XMLNode::as_element).find(|e| attr_eq(e, "tractor", "id", id))
}

fn find_tractor_mut<'a>(root: &'a mut Element, id: &str) -> Option<&'a mut Element> {
    root.children.iter_mut().filter_map(XMLNode::as_mut_element).find(|e| attr_eq(e, "tractor", "id", id))
}

/// Id da `<playlist>` onde os clipes de uma trilha realmente ficam: a
/// primeira das duas playlists que o `<tractor>` da trilha envolve (ver
/// `add_track`; a segunda existe porque o Kdenlive sempre cria o par). Em
/// documentos antigos deste projeto a trilha era a própria `<playlist>`.
fn track_clip_playlist_id(root: &Element, track_id: &str) -> Option<String> {
    match find_tractor(root, track_id) {
        Some(t) => t
            .children
            .iter()
            .filter_map(XMLNode::as_element)
            .find(|e| e.name == "track")?
            .attributes
            .get("producer")
            .cloned(),
        None => find_playlist(root, track_id).map(|_| track_id.to_string()),
    }
}

/// Como `find_playlist`, mas para uma trilha (resolve o `<tractor>` da
/// trilha até a `<playlist>` de clipes dentro dele). Use para qualquer
/// operação de clipe; `find_playlist` sozinho só serve mais para `main_bin`.
pub fn find_track_playlist<'a>(root: &'a Element, track_id: &str) -> Option<&'a Element> {
    let sub_id = track_clip_playlist_id(root, track_id)?;
    find_playlist(root, &sub_id)
}

pub fn find_track_playlist_mut<'a>(root: &'a mut Element, track_id: &str) -> Option<&'a mut Element> {
    let sub_id = track_clip_playlist_id(root, track_id)?;
    find_playlist_mut(root, &sub_id)
}

/// Produtor por id — `<producer>` ou `<chain>` (o Kdenlive grava mídia
/// comum como `<chain>`; os dois têm a mesma forma para este módulo).
pub fn find_producer<'a>(root: &'a Element, id: &str) -> Option<&'a Element> {
    root.children
        .iter()
        .filter_map(XMLNode::as_element)
        .find(|e| attr_eq(e, "producer", "id", id) || attr_eq(e, "chain", "id", id))
}

pub fn find_producer_mut<'a>(root: &'a mut Element, id: &str) -> Option<&'a mut Element> {
    root.children
        .iter_mut()
        .filter_map(XMLNode::as_mut_element)
        .find(|e| attr_eq(e, "producer", "id", id) || attr_eq(e, "chain", "id", id))
}

pub fn main_bin(root: &Element) -> Option<&Element> {
    find_playlist(root, MAIN_BIN_ID)
}

pub fn main_bin_mut(root: &mut Element) -> Option<&mut Element> {
    find_playlist_mut(root, MAIN_BIN_ID)
}

/// Id da sequência: a trilha do tractor do projeto
/// (`kdenlive:projectTractor=1`), como o Kdenlive grava.
fn sequence_tractor_id(root: &Element) -> Option<String> {
    root.children
        .iter()
        .filter_map(XMLNode::as_element)
        .find(|e| e.name == "tractor" && get_property(e, "kdenlive:projectTractor").as_deref() == Some("1"))?
        .children
        .iter()
        .filter_map(XMLNode::as_element)
        .find(|e| e.name == "track")?
        .attributes
        .get("producer")
        .cloned()
}

/// O tractor da timeline (a sequência). Em documentos sem tractor de
/// projeto (formato antigo deste projeto, ou MLT XML comum), o último
/// `<tractor>` entre os filhos de `<mlt>`.
pub fn main_tractor(root: &Element) -> Option<&Element> {
    if let Some(id) = sequence_tractor_id(root)
        && let Some(t) = find_tractor(root, &id)
    {
        return Some(t);
    }
    root.children
        .iter()
        .rev()
        .filter_map(XMLNode::as_element)
        .find(|e| e.name == "tractor")
}

pub fn main_tractor_mut(root: &mut Element) -> Option<&mut Element> {
    if let Some(id) = sequence_tractor_id(root)
        && find_tractor(root, &id).is_some()
    {
        return find_tractor_mut(root, &id);
    }
    root.children
        .iter_mut()
        .rev()
        .filter_map(XMLNode::as_mut_element)
        .find(|e| e.name == "tractor")
}

/// Onde um produtor novo deve entrar em `root.children`: antes da primeira
/// `<playlist>`/`<tractor>`, para ficar definido antes de qualquer trilha
/// ou da bin que o referencie (o XML do MLT só resolve referências a
/// elementos já definidos).
fn producer_insert_index(root: &Element) -> usize {
    root.children
        .iter()
        .position(|n| matches!(n, XMLNode::Element(e) if e.name == "playlist" || e.name == "tractor"))
        .unwrap_or(root.children.len())
}

/// Onde os elementos de uma trilha nova devem entrar: logo antes do tractor
/// da sequência (que vai referenciá-los).
fn sequence_insert_index(root: &Element) -> usize {
    let alvo = sequence_tractor_id(root);
    let posicao = match alvo {
        Some(id) => root.children.iter().position(|n| matches!(n, XMLNode::Element(e) if attr_eq(e, "tractor", "id", &id))),
        None => root.children.iter().rposition(|n| matches!(n, XMLNode::Element(e) if e.name == "tractor")),
    };
    posicao.unwrap_or(root.children.len())
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

/// Próximo número livre no espaço de ids da bin do Kdenlive. Clipes
/// (`kdenlive:id`) e pastas (`kdenlive:folder.<pai>.<id>`) dividem o mesmo
/// espaço: o Kdenlive 23.08 indexa os dois no mesmo mapa ao abrir o
/// projeto, então um id repetido entre um clipe e uma pasta embaralharia a
/// bin.
fn next_bin_numeric_id(root: &Element) -> u64 {
    let mut max_n: u64 = 0;
    for el in root.children.iter().filter_map(XMLNode::as_element) {
        if let Some(n) = get_property(el, "kdenlive:id").and_then(|v| v.parse::<u64>().ok()) {
            max_n = max_n.max(n);
        }
    }
    for (_, id, _) in all_bin_folders(root) {
        if let Ok(n) = id.parse::<u64>() {
            max_n = max_n.max(n);
        }
    }
    max_n + 1
}

/// Dá a um produtor da bin a identidade que o Kdenlive usa para ligá-lo
/// aos clipes da timeline: `kdenlive:id` numérico e `kdenlive:control_uuid`.
fn assign_bin_identity(root: &Element, producer: &mut Element) {
    set_property(producer, "kdenlive:id", &next_bin_numeric_id(root).to_string());
    set_property(producer, "kdenlive:control_uuid", &new_uuid());
}

fn insert_bin_producer(root: &mut Element, producer: Element) -> Result<(), ModelError> {
    let id = producer.attributes.get("id").cloned().unwrap_or_default();
    let insert_at = producer_insert_index(root);
    root.children.insert(insert_at, XMLNode::Element(producer));
    let bin = main_bin_mut(root)
        .ok_or_else(|| ModelError::NotFound(format!("playlist \"{MAIN_BIN_ID}\" não encontrada")))?;
    let mut entry = Element::new("entry");
    entry.attributes.insert("producer".to_string(), id);
    bin.children.push(XMLNode::Element(entry));
    Ok(())
}

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
    let mut producer = element_with_props(
        "producer",
        &[("id", id), ("in", &in_frame.to_string()), ("out", &out_frame.to_string())],
        &[("resource", resource), ("mlt_service", "avformat"), ("kdenlive:clipname", clip_name)],
    );
    assign_bin_identity(root, &mut producer);
    insert_bin_producer(root, producer)?;
    Ok(id.to_string())
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
        root.children.retain(|n| {
            !matches!(n, XMLNode::Element(e) if attr_eq(e, "producer", "id", producer_id) || attr_eq(e, "chain", "id", producer_id))
        });
    }

    Ok(())
}

/// Lista os ids de produtor que estão na bin (referenciados por `main_bin`),
/// sem a própria sequência (que o Kdenlive também lista na bin).
pub fn bin_clip_ids(root: &Element) -> Vec<String> {
    main_bin(root)
        .map(|bin| {
            bin.children
                .iter()
                .filter_map(XMLNode::as_element)
                .filter(|e| e.name == "entry")
                .filter_map(|e| e.attributes.get("producer").cloned())
                .filter(|id| find_tractor(root, id).is_none())
                .collect()
        })
        .unwrap_or_default()
}

/// Cor de um clipe da bin. No Kdenlive a cor de um clipe é uma "tag": a
/// propriedade `kdenlive:tags` guarda cores em hexadecimal separadas por `;`
/// (`TagWidget`/`Bin::slotTagDropped`, código-fonte do Kdenlive). Aceita
/// `#RRGGBB`; um clipe tem uma cor só (a nova substitui a anterior).
pub fn bin_clip_set_color(root: &mut Element, producer_id: &str, color: &str) -> Result<(), ModelError> {
    let limpo = color.trim().to_lowercase();
    let valida = limpo.len() == 7
        && limpo.starts_with('#')
        && limpo[1..].chars().all(|c| c.is_ascii_hexdigit());
    if !valida {
        return Err(ModelError::Invalid(format!(
            "cor inválida: \"{color}\" (o Kdenlive só aceita cores no formato #RRGGBB, ex. \"#ff0000\")"
        )));
    }
    let producer = find_producer_mut(root, producer_id)
        .ok_or_else(|| ModelError::NotFound(format!("clipe não encontrado na bin: {producer_id}")))?;
    set_property(producer, "kdenlive:tags", &limpo);
    Ok(())
}

/// Descrição do clipe da bin (`kdenlive:description`, a "Description" da
/// aba de propriedades do Kdenlive).
pub fn bin_clip_set_note(root: &mut Element, producer_id: &str, note: &str) -> Result<(), ModelError> {
    let producer = find_producer_mut(root, producer_id)
        .ok_or_else(|| ModelError::NotFound(format!("clipe não encontrado na bin: {producer_id}")))?;
    set_property(producer, "kdenlive:description", note);
    Ok(())
}

/// `(cor, nota)` de um clipe da bin, como `bin_clip_set_color`/`set_note`
/// gravam (a cor é a primeira tag do clipe).
pub fn bin_clip_color_note(producer: &Element) -> (Option<String>, Option<String>) {
    let cor = get_property(producer, "kdenlive:tags")
        .and_then(|t| t.split(';').next().map(str::to_string))
        .filter(|c| !c.is_empty());
    (cor, get_property(producer, "kdenlive:description").filter(|n| !n.is_empty()))
}

/// Renomeia um clipe da bin (propriedade `kdenlive:clipname`, confirmada
/// no `dev-docs/fileformat.md` do Kdenlive).
pub fn bin_clip_rename(root: &mut Element, producer_id: &str, nome: &str) -> Result<(), ModelError> {
    let producer = find_producer_mut(root, producer_id)
        .ok_or_else(|| ModelError::NotFound(format!("clipe não encontrado na bin: {producer_id}")))?;
    set_property(producer, "kdenlive:clipname", nome);
    Ok(())
}

/// Move um clipe da bin para dentro de uma pasta (ou para a raiz, se
/// `folder_id` for `None`) — propriedade `kdenlive:folderid`, confirmada
/// no `dev-docs/fileformat.md` do Kdenlive.
pub fn bin_move_clip(root: &mut Element, producer_id: &str, folder_id: Option<&str>) -> Result<(), ModelError> {
    let producer = find_producer_mut(root, producer_id)
        .ok_or_else(|| ModelError::NotFound(format!("clipe não encontrado na bin: {producer_id}")))?;
    match folder_id {
        Some(id) => set_property(producer, "kdenlive:folderid", id),
        None => remove_property(producer, "kdenlive:folderid"),
    }
    Ok(())
}

/// Cria uma pasta na bin. Convenção confirmada no `dev-docs/fileformat.md`
/// do Kdenlive: uma propriedade `kdenlive:folder.<id do pai>.<id da nova
/// pasta>` = nome, na playlist `main_bin` (`-1` como pai = raiz). Devolve o
/// id da pasta criada.
pub fn create_bin_folder(root: &mut Element, parent_id: Option<&str>, nome: &str) -> Result<String, ModelError> {
    let folder_id = next_bin_numeric_id(root).to_string();
    let bin = main_bin_mut(root)
        .ok_or_else(|| ModelError::NotFound(format!("playlist \"{MAIN_BIN_ID}\" não encontrada")))?;
    let prop_name = format!("kdenlive:folder.{}.{folder_id}", parent_id.unwrap_or("-1"));
    set_property(bin, &prop_name, nome);
    Ok(folder_id)
}

fn all_bin_folders(root: &Element) -> Vec<(String, String, String)> {
    let Some(bin) = main_bin(root) else {
        return Vec::new();
    };
    bin.children
        .iter()
        .filter_map(XMLNode::as_element)
        .filter(|e| e.name == "property")
        .filter_map(|e| {
            let nome_prop = e.attributes.get("name")?;
            let resto = nome_prop.strip_prefix("kdenlive:folder.")?;
            let (pai, filho) = resto.rsplit_once('.')?;
            let texto = e.get_text()?.into_owned();
            Some((pai.to_string(), filho.to_string(), texto))
        })
        .collect()
}

/// `(id do pai, id da pasta, nome)` de cada pasta da bin, sem a pasta de
/// sequências que o próprio Kdenlive administra (`kdenlive:sequenceFolder`).
pub fn list_bin_folders(root: &Element) -> Vec<(String, String, String)> {
    let pasta_de_sequencias = main_bin(root).and_then(|b| get_property(b, "kdenlive:sequenceFolder"));
    all_bin_folders(root)
        .into_iter()
        .filter(|(_, id, _)| Some(id) != pasta_de_sequencias.as_ref())
        .collect()
}

// ---------------------------------------------------------------------------
// Trilhas e clipes da timeline
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrackKind {
    Video,
    Audio,
}

/// Ids, na ordem do MLT, de todos os `<track>` da sequência — inclusive a
/// black track (índice 0 nos documentos no formato do Kdenlive). É nesse
/// índice que `a_track`/`b_track` das transições se baseiam.
fn sequence_tracks(root: &Element) -> Vec<String> {
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

fn is_black_track(root: &Element, id: &str) -> bool {
    id == BLACK_TRACK_ID
        || find_producer(root, id).is_some_and(|p| get_property(p, "kdenlive:playlistid").as_deref() == Some("black_track"))
}

fn has_black_track(root: &Element) -> bool {
    sequence_tracks(root).first().is_some_and(|id| is_black_track(root, id))
}

/// Índice MLT de uma trilha (posição entre os `<track>` da sequência,
/// contando a black track).
fn track_index(root: &Element, track_id: &str) -> Option<usize> {
    sequence_tracks(root).iter().position(|id| id == track_id)
}

fn is_internal_transition(e: &Element) -> bool {
    e.name == "transition" && get_property(e, "internal_added").is_some()
}

/// Lê `a_track`/`b_track` (propriedade ou atributo) de uma transição.
fn transition_track(e: &Element, nome: &str) -> Option<usize> {
    get_property(e, nome).or_else(|| e.attributes.get(nome).cloned())?.parse().ok()
}

/// Reaplica `f` aos índices `a_track`/`b_track` das transições do usuário
/// (as internas são recriadas por `rebuild_internal_transitions`).
/// Transições para as quais `f` devolve `None` são removidas.
fn remap_user_transitions(root: &mut Element, f: impl Fn(usize) -> Option<usize>) {
    let Some(tractor) = main_tractor_mut(root) else { return };
    tractor.children.retain_mut(|n| {
        let XMLNode::Element(e) = n else { return true };
        if e.name != "transition" || is_internal_transition(e) {
            return true;
        }
        for nome in ["a_track", "b_track"] {
            if let Some(atual) = transition_track(e, nome) {
                match f(atual) {
                    Some(novo) => {
                        e.attributes.shift_remove(nome);
                        set_property(e, nome, &novo.to_string());
                    }
                    None => return false,
                }
            }
        }
        true
    });
}

/// Recria as transições internas de composição de trilha, como o Kdenlive:
/// para cada trilha acima da black track, um `mix` (áudio) ou um `qtblend`
/// (vídeo) com `a_track=0`. Os `mix` ficam antes das transições do usuário
/// e os `qtblend` depois, na mesma ordem em que o Kdenlive as planta
/// (`TimelineModel::replantCompositions`) — é isso que faz o libmlt
/// misturar o áudio de todas as trilhas e sobrepor os vídeos respeitando a
/// transparência (ex. um título por cima do vídeo).
fn rebuild_internal_transitions(root: &mut Element) {
    if !has_black_track(root) {
        return;
    }
    let tipos: Vec<(usize, TrackKind)> = sequence_tracks(root)
        .iter()
        .enumerate()
        .skip(1)
        .map(|(i, id)| (i, track_kind(root, id).unwrap_or(TrackKind::Video)))
        .collect();
    let Some(tractor) = main_tractor_mut(root) else { return };
    tractor.children.retain(|n| !matches!(n, XMLNode::Element(e) if is_internal_transition(e)));

    let primeira_do_usuario = tractor
        .children
        .iter()
        .position(|n| matches!(n, XMLNode::Element(e) if e.name == "transition"))
        .unwrap_or(tractor.children.len());
    let mut mixes = Vec::new();
    for (i, tipo) in &tipos {
        let b = i.to_string();
        match tipo {
            TrackKind::Audio => mixes.push(XMLNode::Element(element_with_props(
                "transition",
                &[("id", &format!("internal_mix{i}"))],
                &[
                    ("a_track", "0"),
                    ("b_track", &b),
                    ("mlt_service", "mix"),
                    ("kdenlive_id", "mix"),
                    ("internal_added", INTERNAL_ADDED),
                    ("always_active", "1"),
                    ("accepts_blanks", "1"),
                    ("sum", "1"),
                ],
            ))),
            TrackKind::Video => tractor.children.push(XMLNode::Element(element_with_props(
                "transition",
                &[("id", &format!("internal_blend{i}"))],
                &[
                    ("a_track", "0"),
                    ("b_track", &b),
                    ("compositing", "0"),
                    ("distort", "0"),
                    ("rotate_center", "0"),
                    ("mlt_service", "qtblend"),
                    ("kdenlive_id", "qtblend"),
                    ("internal_added", INTERNAL_ADDED),
                    ("always_active", "1"),
                ],
            ))),
        }
    }
    for (k, m) in mixes.into_iter().enumerate() {
        tractor.children.insert(primeira_do_usuario + k, m);
    }
}

/// Cria uma trilha nova e devolve seu id. Como no Kdenlive real (D-039/
/// D-040 em docs/decisoes.md), uma trilha é um `<tractor id="{id}">`
/// envolvendo duas `<playlist>` (só a primeira recebe clipes; a segunda
/// existe porque o Kdenlive sempre cria o par), mais um `<track>` na
/// sequência. Trilhas de áudio entram logo acima da última trilha de áudio
/// (ou da black track), trilhas de vídeo no topo — a mesma organização do
/// Kdenlive, que espera as de áudio embaixo.
pub fn add_track(root: &mut Element, id: &str, kind: TrackKind) -> Result<String, ModelError> {
    if find_tractor(root, id).is_some() || find_playlist(root, id).is_some() {
        return Err(ModelError::Invalid(format!("já existe um elemento com o id \"{id}\"")));
    }
    let track_id = id.to_string();
    let playlist_a_id = format!("{track_id}_a");
    let playlist_b_id = format!("{track_id}_b");
    // Trilha de vídeo: as duas sub-playlists só contribuem vídeo (áudio
    // oculto); trilha de áudio, o oposto — padrão dos projetos reais.
    let hide_sub = match kind {
        TrackKind::Video => "audio",
        TrackKind::Audio => "video",
    };

    let mut playlist_a = element_with_props("playlist", &[("id", &playlist_a_id)], &[]);
    let mut playlist_b = element_with_props("playlist", &[("id", &playlist_b_id)], &[]);
    let mut track_tractor = element_with_props("tractor", &[("id", &track_id), ("in", "0")], &[]);
    // O tipo também fica numa propriedade própria deste projeto, para
    // `track_kind` não depender só de `kdenlive:audio_track` (que só existe
    // em trilhas de áudio).
    set_property(
        &mut track_tractor,
        "kdenlive-mcp-plus:kind",
        match kind {
            TrackKind::Video => "video",
            TrackKind::Audio => "audio",
        },
    );
    if kind == TrackKind::Audio {
        set_property(&mut playlist_a, "kdenlive:audio_track", "1");
        set_property(&mut playlist_b, "kdenlive:audio_track", "1");
        set_property(&mut track_tractor, "kdenlive:audio_track", "1");
    }
    for sub in [&playlist_a_id, &playlist_b_id] {
        track_tractor
            .children
            .push(XMLNode::Element(element_with_props("track", &[("hide", hide_sub), ("producer", sub)], &[])));
    }

    let insert_at = sequence_insert_index(root);
    root.children.insert(insert_at, XMLNode::Element(playlist_a));
    root.children.insert(insert_at + 1, XMLNode::Element(playlist_b));
    root.children.insert(insert_at + 2, XMLNode::Element(track_tractor));

    let todas = sequence_tracks(root);
    let base = usize::from(has_black_track(root));
    let nova_posicao = match kind {
        TrackKind::Video => todas.len(),
        TrackKind::Audio => todas
            .iter()
            .enumerate()
            .skip(base)
            .filter(|(_, t)| track_kind(root, t) == Some(TrackKind::Audio))
            .map(|(i, _)| i + 1)
            .next_back()
            .unwrap_or(base),
    };
    remap_user_transitions(root, |i| Some(if i >= nova_posicao { i + 1 } else { i }));

    let mut track = element_with_props("track", &[("producer", &track_id)], &[]);
    if kind == TrackKind::Audio {
        track.attributes.insert("hide".to_string(), "video".to_string());
    }
    let tractor = main_tractor_mut(root)
        .ok_or_else(|| ModelError::NotFound("tractor da sequência não encontrado".to_string()))?;
    let posicoes: Vec<usize> = tractor
        .children
        .iter()
        .enumerate()
        .filter(|(_, n)| matches!(n, XMLNode::Element(e) if e.name == "track"))
        .map(|(i, _)| i)
        .collect();
    let indice_filho = match posicoes.get(nova_posicao) {
        Some(&i) => i,
        None => posicoes.last().map(|i| i + 1).unwrap_or(tractor.children.len()),
    };
    tractor.children.insert(indice_filho, XMLNode::Element(track));

    rebuild_internal_transitions(root);
    Ok(track_id)
}

/// Remove uma trilha (o `<track>` da sequência, o `<tractor>` da trilha e
/// suas `<playlist>`, com todos os clipes nelas). Transições do usuário que
/// usavam essa trilha também são removidas; as outras são reindexadas.
/// Nenhum produtor é apagado por esta operação.
pub fn remove_track(root: &mut Element, track_id: &str) -> Result<(), ModelError> {
    let indice = track_index(root, track_id)
        .filter(|_| !is_black_track(root, track_id))
        .ok_or_else(|| ModelError::NotFound(format!("trilha não encontrada: {track_id}")))?;
    let sub_playlists: Vec<String> = find_tractor(root, track_id)
        .map(|t| {
            t.children
                .iter()
                .filter_map(XMLNode::as_element)
                .filter(|e| e.name == "track")
                .filter_map(|e| e.attributes.get("producer").cloned())
                .collect()
        })
        .unwrap_or_else(|| vec![track_id.to_string()]);

    remap_user_transitions(root, |i| match i.cmp(&indice) {
        std::cmp::Ordering::Less => Some(i),
        std::cmp::Ordering::Equal => None,
        std::cmp::Ordering::Greater => Some(i - 1),
    });
    if let Some(tractor) = main_tractor_mut(root) {
        tractor
            .children
            .retain(|n| !matches!(n, XMLNode::Element(e) if attr_eq(e, "track", "producer", track_id)));
    }
    root.children.retain(|n| {
        !matches!(n, XMLNode::Element(e) if attr_eq(e, "tractor", "id", track_id)
            || (e.name == "playlist" && e.attributes.get("id").is_some_and(|id| sub_playlists.contains(id))))
    });
    rebuild_internal_transitions(root);
    Ok(())
}

/// Lista os ids de trilha, na ordem em que aparecem na sequência (primeiro =
/// mais ao fundo, último = mais acima — convenção do MLT), sem a black
/// track.
pub fn track_ids(root: &Element) -> Vec<String> {
    sequence_tracks(root).into_iter().filter(|id| !is_black_track(root, id)).collect()
}

pub fn track_kind(root: &Element, track_id: &str) -> Option<TrackKind> {
    let el = find_tractor(root, track_id).or_else(|| find_playlist(root, track_id))?;
    Some(match get_property(el, "kdenlive-mcp-plus:kind").as_deref() {
        Some("audio") => TrackKind::Audio,
        Some("video") => TrackKind::Video,
        _ if get_property(el, "kdenlive:audio_track").as_deref() == Some("1") => TrackKind::Audio,
        _ => TrackKind::Video,
    })
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

/// Nome de exibição da trilha (`kdenlive:track_name`, a propriedade que o
/// próprio Kdenlive usa, no tractor da trilha).
pub fn track_name(root: &Element, track_id: &str) -> Option<String> {
    let el = find_tractor(root, track_id).or_else(|| find_playlist(root, track_id))?;
    get_property(el, "kdenlive:track_name").or_else(|| get_property(el, "kdenlive-mcp-plus:name"))
}

pub fn track_set_name(root: &mut Element, track_id: &str, nome: &str) -> Result<(), ModelError> {
    let el = find_tractor_mut(root, track_id)
        .ok_or_else(|| ModelError::NotFound(format!("trilha não encontrada: {track_id}")))?;
    set_property(el, "kdenlive:track_name", nome);
    Ok(())
}

/// Trava/destrava a trilha (`kdenlive:locked_track`, lida pelo Kdenlive ao
/// abrir; conceito só de edição, não afeta o render).
pub fn track_set_locked(root: &mut Element, track_id: &str, locked: bool) -> Result<(), ModelError> {
    let el = find_tractor_mut(root, track_id)
        .ok_or_else(|| ModelError::NotFound(format!("trilha não encontrada: {track_id}")))?;
    set_property(el, "kdenlive:locked_track", if locked { "1" } else { "0" });
    Ok(())
}

pub fn track_is_locked(root: &Element, track_id: &str) -> bool {
    find_tractor(root, track_id)
        .or_else(|| find_playlist(root, track_id))
        .and_then(|p| get_property(p, "kdenlive:locked_track").or_else(|| get_property(p, "kdenlive-mcp-plus:locked")))
        .as_deref()
        == Some("1")
}

/// Move a trilha uma posição para baixo na ordem do MLT (troca com a
/// anterior). Erro se já for a primeira (a black track nunca se move).
pub fn track_move_up(root: &mut Element, track_id: &str) -> Result<(), ModelError> {
    move_track(root, track_id, -1)
}

/// Move a trilha uma posição para cima na ordem do MLT (troca com a próxima).
pub fn track_move_down(root: &mut Element, track_id: &str) -> Result<(), ModelError> {
    move_track(root, track_id, 1)
}

fn move_track(root: &mut Element, track_id: &str, delta: isize) -> Result<(), ModelError> {
    let ids = track_ids(root);
    let posicao = ids
        .iter()
        .position(|id| id == track_id)
        .ok_or_else(|| ModelError::NotFound(format!("trilha não encontrada: {track_id}")))?;
    let nova_posicao = posicao as isize + delta;
    if nova_posicao < 0 || nova_posicao as usize >= ids.len() {
        return Err(ModelError::Invalid(format!(
            "a trilha \"{track_id}\" já está {}",
            if delta < 0 { "na primeira posição" } else { "na última posição" }
        )));
    }
    let outra = ids[nova_posicao as usize].clone();
    let (Some(a), Some(b)) = (track_index(root, track_id), track_index(root, &outra)) else {
        return Err(ModelError::NotFound(format!("trilha não encontrada: {track_id}")));
    };
    let tractor = main_tractor_mut(root)
        .ok_or_else(|| ModelError::NotFound("tractor da sequência não encontrado".to_string()))?;
    let filhos: Vec<usize> = tractor
        .children
        .iter()
        .enumerate()
        .filter(|(_, n)| matches!(n, XMLNode::Element(e) if e.name == "track"))
        .map(|(i, _)| i)
        .collect();
    tractor.children.swap(filhos[a], filhos[b]);
    remap_user_transitions(root, |i| Some(if i == a { b } else if i == b { a } else { i }));
    rebuild_internal_transitions(root);
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

/// Garante espaço para `node` (um `<entry>` ou `<blank>` já montado, com
/// seus próprios atributos) exatamente em `position` dentro de `playlist`,
/// e o insere lá. Devolve o índice lógico (contando só `entry`/`blank`) do
/// nó inserido.
///
/// Comportamento:
/// - `position` além do fim atual: cria um `<blank>` de preenchimento até
///   lá (se precisar) e acrescenta `node` no fim.
/// - `position` cai exatamente no **início** de um `<entry>`/`<blank>`
///   existente: `node` é inserido ali, **antes** dele — como as posições
///   do MLT são implícitas (soma dos comprimentos anteriores), isso
///   empurra esse slot e todos os seguintes para depois, sem precisar
///   reescrevê-los. É o único "deslocamento" que este projeto faz sozinho.
/// - `position` cai **dentro** de um `<blank>` (não no início): o `<blank>`
///   é dividido em dois, com `node` entre eles.
/// - `position` cai **dentro** de um `<entry>` (não no início): erro — para
///   caber ali seria preciso dividir esse clipe também, o que esta fase
///   não faz.
fn insert_node_at(playlist: &mut Element, position: i64, node: Element) -> Result<usize, ModelError> {
    if position < 0 {
        return Err(ModelError::Invalid("a posição não pode ser negativa".to_string()));
    }
    let total = track_length(playlist);

    if position >= total {
        if position > total {
            let mut blank = Element::new("blank");
            blank.attributes.insert("length".to_string(), (position - total).to_string());
            playlist.children.push(XMLNode::Element(blank));
        }
        playlist.children.push(XMLNode::Element(node));
        return Ok(contar_slots(playlist) - 1);
    }

    // Acha o slot que contém `position` (0 <= position < total, já garantido acima).
    let mut acumulado: i64 = 0;
    let mut alvo = 0usize;
    for (i, child) in playlist.children.iter().enumerate() {
        if let XMLNode::Element(e) = child
            && (e.name == "entry" || e.name == "blank")
        {
            let len = entry_length(e);
            if position < acumulado + len {
                alvo = i;
                break;
            }
            acumulado += len;
        }
    }
    let slot_comeco = acumulado;
    let antes = position - slot_comeco;
    let existente = playlist.children[alvo].as_element().cloned().expect("é um Element (filtrado acima)");

    if existente.name == "entry" {
        if antes == 0 {
            playlist.children.insert(alvo, XMLNode::Element(node));
            return Ok(contar_slots_antes(playlist, alvo));
        }
        return Err(ModelError::Invalid(format!(
            "a posição {position} cai dentro de um clipe já existente; esta fase não divide um \
             clipe para abrir espaço (\"ripple\" parcial não suportado)"
        )));
    }

    // `existente` é um `<blank>`: divide em até dois blanks ao redor do novo nó.
    let blank_len = entry_length(&existente);
    let depois = blank_len - antes;
    playlist.children.remove(alvo);
    let mut novos = Vec::with_capacity(3);
    if antes > 0 {
        let mut b = Element::new("blank");
        b.attributes.insert("length".to_string(), antes.to_string());
        novos.push(XMLNode::Element(b));
    }
    let posicao_bruta_do_node = alvo + novos.len();
    novos.push(XMLNode::Element(node));
    if depois > 0 {
        let mut b = Element::new("blank");
        b.attributes.insert("length".to_string(), depois.to_string());
        novos.push(XMLNode::Element(b));
    }
    for (deslocamento, n) in novos.into_iter().enumerate() {
        playlist.children.insert(alvo + deslocamento, n);
    }
    Ok(contar_slots_antes(playlist, posicao_bruta_do_node))
}

fn contar_slots(playlist: &Element) -> usize {
    playlist
        .children
        .iter()
        .filter(|n| matches!(n, XMLNode::Element(e) if e.name == "entry" || e.name == "blank"))
        .count()
}

/// Quantos slots (`entry`/`blank`) existem antes do índice bruto
/// `indice_bruto` em `playlist.children` — ou seja, o índice lógico do
/// slot que está exatamente em `indice_bruto`.
fn contar_slots_antes(playlist: &Element, indice_bruto: usize) -> usize {
    playlist.children[..indice_bruto]
        .iter()
        .filter(|n| matches!(n, XMLNode::Element(e) if e.name == "entry" || e.name == "blank"))
        .count()
}

/// Posição absoluta (em quadros) de onde o slot lógico `slot_index` começa.
fn position_of_slot(playlist: &Element, slot_index: usize) -> i64 {
    playlist
        .children
        .iter()
        .filter_map(XMLNode::as_element)
        .filter(|e| e.name == "entry" || e.name == "blank")
        .take(slot_index)
        .map(entry_length)
        .sum()
}

/// Posiciona um clipe (produtor já existente, da bin ou não) numa trilha.
/// Ver `insert_node_at` para a semântica exata de `position`. Devolve o
/// índice da entrada criada (para montar a referência de clipe
/// `"<track_id>:<índice>"`).
pub fn add_clip_to_track(
    root: &mut Element,
    track_id: &str,
    producer_id: &str,
    clip_in: i64,
    clip_out: i64,
    position: i64,
) -> Result<usize, ModelError> {
    let producer = find_producer(root, producer_id)
        .ok_or_else(|| ModelError::NotFound(format!("produtor não encontrado: {producer_id}")))?;
    if clip_out < clip_in {
        return Err(ModelError::Invalid(format!(
            "out ({clip_out}) não pode ser menor que in ({clip_in})"
        )));
    }
    // O Kdenlive real espera que a entrada da timeline carregue o
    // `kdenlive:id` do produtor, não só o atributo `producer=` do MLT — sem
    // isso ele trata o clipe como "sem referência ao grupo" e o remove ao
    // abrir o projeto (achado empírico, D-039 em docs/decisoes.md).
    let kdenlive_id = get_property(producer, "kdenlive:id").unwrap_or_else(|| producer_id.to_string());

    let playlist = find_track_playlist_mut(root, track_id)
        .ok_or_else(|| ModelError::NotFound(format!("trilha não encontrada: {track_id}")))?;

    let mut entry = Element::new("entry");
    entry.attributes.insert("producer".to_string(), producer_id.to_string());
    entry.attributes.insert("in".to_string(), clip_in.to_string());
    entry.attributes.insert("out".to_string(), clip_out.to_string());
    set_property(&mut entry, "kdenlive:id", &kdenlive_id);
    insert_node_at(playlist, position, entry)
}

/// Remove o clipe no índice `index` da trilha `track_id` (vira um `<blank>`
/// do mesmo tamanho, para não deslocar os clipes seguintes).
pub fn remove_clip_from_track(root: &mut Element, track_id: &str, index: usize) -> Result<(), ModelError> {
    let playlist = find_track_playlist_mut(root, track_id)
        .ok_or_else(|| ModelError::NotFound(format!("trilha não encontrada: {track_id}")))?;
    let slot_index = nth_slot_index(playlist, index)
        .ok_or_else(|| ModelError::NotFound(format!("clipe não encontrado: {track_id}:{index}")))?;
    let length = entry_length(playlist.children[slot_index].as_element().expect("é um Element"));
    let mut blank = Element::new("blank");
    blank.attributes.insert("length".to_string(), length.to_string());
    playlist.children[slot_index] = XMLNode::Element(blank);
    Ok(())
}

/// Reposiciona um clipe já existente na mesma trilha, para uma nova
/// posição (em quadros). O slot antigo vira um `<blank>` (como
/// `remove_clip_from_track`) e o clipe é reinserido em `new_position` (ver
/// `insert_node_at`). Devolve o novo índice do clipe.
pub fn move_clip(root: &mut Element, track_id: &str, index: usize, new_position: i64) -> Result<usize, ModelError> {
    let entry = get_clip(root, track_id, index)
        .ok_or_else(|| ModelError::NotFound(format!("clipe não encontrado: {track_id}:{index}")))?;
    let producer_id = entry
        .attributes
        .get("producer")
        .cloned()
        .ok_or_else(|| ModelError::Invalid("entrada sem produtor".to_string()))?;
    let in_f: i64 = entry.attributes.get("in").and_then(|v| v.parse().ok()).unwrap_or(0);
    let out_f: i64 = entry.attributes.get("out").and_then(|v| v.parse().ok()).unwrap_or(0);

    remove_clip_from_track(root, track_id, index)?;
    add_clip_to_track(root, track_id, &producer_id, in_f, out_f, new_position)
}

/// Ajusta as alças de entrada/saída (`in`/`out`) de um clipe já na
/// timeline — não é o filtro de recorte de imagem, é o trecho usado do
/// produtor. Como as posições do MLT são implícitas, encurtar/alongar o
/// clipe desloca naturalmente tudo que vem depois dele na mesma trilha.
pub fn trim_clip(root: &mut Element, track_id: &str, index: usize, new_in: i64, new_out: i64) -> Result<(), ModelError> {
    if new_out < new_in {
        return Err(ModelError::Invalid(format!("out ({new_out}) não pode ser menor que in ({new_in})")));
    }
    let entry = get_clip_mut(root, track_id, index)
        .ok_or_else(|| ModelError::NotFound(format!("clipe não encontrado: {track_id}:{index}")))?;
    entry.attributes.insert("in".to_string(), new_in.to_string());
    entry.attributes.insert("out".to_string(), new_out.to_string());
    Ok(())
}

/// Corta um clipe em dois, em `split_at` — um quadro do **produtor**
/// (então `in < split_at <= out`, não uma posição absoluta na timeline).
/// O primeiro pedaço fica no mesmo índice, encurtado para `[in, split_at
/// - 1]`; o segundo pedaço, `[split_at, out]`, é inserido logo depois.
/// Devolve `(índice do primeiro pedaço, índice do segundo pedaço)`.
pub fn split_clip(root: &mut Element, track_id: &str, index: usize, split_at: i64) -> Result<(usize, usize), ModelError> {
    let entry = get_clip(root, track_id, index)
        .ok_or_else(|| ModelError::NotFound(format!("clipe não encontrado: {track_id}:{index}")))?;
    let producer_id = entry
        .attributes
        .get("producer")
        .cloned()
        .ok_or_else(|| ModelError::Invalid("entrada sem produtor".to_string()))?;
    let in_f: i64 = entry.attributes.get("in").and_then(|v| v.parse().ok()).unwrap_or(0);
    let out_f: i64 = entry.attributes.get("out").and_then(|v| v.parse().ok()).unwrap_or(0);

    if split_at <= in_f || split_at > out_f {
        return Err(ModelError::Invalid(format!(
            "split_at ({split_at}) precisa estar entre {in_f} (exclusive) e {out_f} (inclusive)"
        )));
    }

    let playlist = find_track_playlist(root, track_id)
        .ok_or_else(|| ModelError::NotFound(format!("trilha não encontrada: {track_id}")))?;
    let posicao_do_corte = position_of_slot(playlist, index) + (split_at - in_f);

    let entry_mut = get_clip_mut(root, track_id, index).expect("já confirmado acima que existe");
    entry_mut.attributes.insert("out".to_string(), (split_at - 1).to_string());

    let segundo_indice = add_clip_to_track(root, track_id, &producer_id, split_at, out_f, posicao_do_corte)?;
    Ok((index, segundo_indice))
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
    let playlist = find_track_playlist(root, track_id)?;
    let slot_index = nth_slot_index(playlist, index)?;
    match &playlist.children[slot_index] {
        XMLNode::Element(e) if e.name == "entry" => Some(e),
        _ => None,
    }
}

pub fn get_clip_mut<'a>(root: &'a mut Element, track_id: &str, index: usize) -> Option<&'a mut Element> {
    let playlist = find_track_playlist_mut(root, track_id)?;
    let slot_index = nth_slot_index(playlist, index)?;
    match playlist.children.get_mut(slot_index) {
        Some(XMLNode::Element(e)) if e.name == "entry" => Some(e),
        _ => None,
    }
}

/// Lista `(índice, producer_id, in, out)` dos clipes (não `<blank>`) de uma trilha.
pub fn list_clips(root: &Element, track_id: &str) -> Vec<(usize, String, i64, i64)> {
    let Some(playlist) = find_track_playlist(root, track_id) else {
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

/// Como `list_clips`, mas também traz a posição absoluta (em quadros) de
/// cada clipe na trilha: `(índice, producer_id, in, out, position)`.
pub fn list_clips_with_position(root: &Element, track_id: &str) -> Vec<(usize, String, i64, i64, i64)> {
    let Some(playlist) = find_track_playlist(root, track_id) else {
        return Vec::new();
    };
    list_clips(root, track_id)
        .into_iter()
        .map(|(indice, producer, in_f, out_f)| (indice, producer, in_f, out_f, position_of_slot(playlist, indice)))
        .collect()
}

/// Duração total do projeto: o maior fim de trilha entre todas as trilhas.
pub fn project_duration(root: &Element) -> i64 {
    track_ids(root)
        .iter()
        .filter_map(|id| find_track_playlist(root, id))
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
    let identidade: Vec<(&str, String)> = ["kdenlive:id", "kdenlive:control_uuid"]
        .into_iter()
        .filter_map(|k| get_property(original, k).map(|v| (k, v)))
        .collect();

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
    // Como o Kdenlive grava um clipe com velocidade: `warp_*` (de onde ele
    // lê a velocidade, `projectclip.cpp`) e a mesma identidade do clipe da
    // bin, para ser religado a ele ao abrir.
    set_property(&mut wrapper, "warp_speed", &velocidade.to_string());
    set_property(&mut wrapper, "warp_resource", &recurso_original);
    for chave in ["kdenlive:id", "kdenlive:control_uuid"] {
        if let Some(v) = identidade.iter().find(|(k, _)| *k == chave).map(|(_, v)| v.clone()) {
            set_property(&mut wrapper, chave, &v);
        }
    }

    let insert_at = producer_insert_index(root);
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
    set_property(&mut filtro, "mlt_service", service);
    // Sem a propriedade `kdenlive_id` (note: sem dois-pontos — convenção
    // diferente de `kdenlive:*`), o Kdenlive real marca o filtro como
    // "efeito ausente" e o remove ao abrir o projeto, mesmo que o
    // `mlt_service` seja válido para o libmlt. Confirmado em projetos reais
    // do Kdenlive: para os serviços simples que este projeto usa
    // (`volume`, `brightness`, `qtblend`), o valor é igual ao do próprio
    // `mlt_service`. Ver D-039 em docs/decisoes.md.
    set_property(&mut filtro, "kdenlive_id", service);
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
        .filter_map(|e| Some((e.attributes.get("id")?.clone(), service_of(e)?)))
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

/// Lê o valor atual de um parâmetro de um filtro de um clipe (`None` se o
/// filtro não existir ou o parâmetro nunca tiver sido definido).
pub fn get_filter_param(root: &Element, track_id: &str, index: usize, filter_id: &str, param: &str) -> Option<String> {
    let entry = get_clip(root, track_id, index)?;
    let filtro = entry
        .children
        .iter()
        .filter_map(XMLNode::as_element)
        .find(|e| attr_eq(e, "filter", "id", filter_id))?;
    get_property(filtro, param)
}

// ---------------------------------------------------------------------------
// Keyframes de um parâmetro de filtro — formato de propriedade animada do
// próprio MLT (`"quadro=valor;quadro=valor;..."`, confirmado renderizando
// um `brightness.level` com keyframes neste ambiente; ver docs/06-testes.md).
// ---------------------------------------------------------------------------

fn parse_keyframes(valor: &str) -> Vec<(i64, String)> {
    valor
        .split(';')
        .filter_map(|parte| {
            let parte = parte.trim();
            if parte.is_empty() {
                return None;
            }
            let (quadro, v) = parte.split_once('=')?;
            let quadro: i64 = quadro.trim().parse().ok()?;
            Some((quadro, v.trim().to_string()))
        })
        .collect()
}

fn format_keyframes(mut pares: Vec<(i64, String)>) -> String {
    pares.sort_by_key(|(quadro, _)| *quadro);
    pares
        .into_iter()
        .map(|(quadro, valor)| format!("{quadro}={valor}"))
        .collect::<Vec<_>>()
        .join(";")
}

/// Define o valor de um parâmetro de filtro num quadro específico,
/// preservando os outros keyframes já existentes (substitui o keyframe
/// daquele quadro exato, se já houver um).
pub fn keyframe_set(
    root: &mut Element,
    track_id: &str,
    index: usize,
    filter_id: &str,
    param: &str,
    frame: i64,
    value: &str,
) -> Result<(), ModelError> {
    let atual = get_filter_param(root, track_id, index, filter_id, param).unwrap_or_default();
    let mut pares = parse_keyframes(&atual);
    pares.retain(|(f, _)| *f != frame);
    pares.push((frame, value.to_string()));
    set_filter_param(root, track_id, index, filter_id, param, &format_keyframes(pares))
}

/// Lista os keyframes `(quadro, valor)` de um parâmetro, em ordem.
pub fn keyframe_list(root: &Element, track_id: &str, index: usize, filter_id: &str, param: &str) -> Vec<(i64, String)> {
    let Some(atual) = get_filter_param(root, track_id, index, filter_id, param) else {
        return Vec::new();
    };
    let mut pares = parse_keyframes(&atual);
    pares.sort_by_key(|(f, _)| *f);
    pares
}

/// Remove o keyframe de um quadro específico.
pub fn keyframe_remove(
    root: &mut Element,
    track_id: &str,
    index: usize,
    filter_id: &str,
    param: &str,
    frame: i64,
) -> Result<(), ModelError> {
    let atual = get_filter_param(root, track_id, index, filter_id, param).unwrap_or_default();
    let mut pares = parse_keyframes(&atual);
    let antes = pares.len();
    pares.retain(|(f, _)| *f != frame);
    if pares.len() == antes {
        return Err(ModelError::NotFound(format!("nenhum keyframe no quadro {frame}")));
    }
    set_filter_param(root, track_id, index, filter_id, param, &format_keyframes(pares))
}

// ---------------------------------------------------------------------------
// Títulos: produtor `kdenlivetitle` (o titulador do próprio Kdenlive, também
// disponível no libmlt — confirmado com `melt -query producer=kdenlivetitle`),
// com o conteúdo num XML `<kdenlivetitle>` guardado na propriedade `xmldata`.
// Esquema confirmado no código-fonte do Kdenlive (`TitleDocument::xml`,
// titledocument.cpp). Abre no Kdenlive como título editável no titulador.
// Documentos antigos deste projeto usavam o produtor `pango`, que o
// Kdenlive não reconhece (o título aparecia como "INVALID"): continuam
// legíveis/editáveis aqui, mas títulos novos são sempre `kdenlivetitle`.
// ---------------------------------------------------------------------------

/// Texto, fonte, tamanho (px) e cor (`R,G,B,A`) de um título.
#[derive(Debug, Clone, PartialEq)]
pub struct TitleSpec {
    pub text: String,
    pub font_family: String,
    pub font_size: i64,
    pub color_rgba: String,
}

/// Converte `#RRGGBB`, `#RRGGBBAA`, `0xRRGGBB` ou `0xRRGGBBAA` para o
/// formato `R,G,B,A` do titulador do Kdenlive (`TitleDocument::colorToString`).
fn color_to_rgba(cor: &str) -> Result<String, ModelError> {
    let limpo = cor.trim();
    let hex = limpo
        .strip_prefix('#')
        .or_else(|| limpo.strip_prefix("0x"))
        .or_else(|| limpo.strip_prefix("0X"))
        .unwrap_or(limpo);
    let byte = |i: usize| u8::from_str_radix(hex.get(i..i + 2).unwrap_or("zz"), 16);
    let invalida = || ModelError::Invalid(format!("cor inválida: \"{cor}\" (use #RRGGBB, #RRGGBBAA ou 0xRRGGBBAA)"));
    let (r, g, b) = (byte(0).map_err(|_| invalida())?, byte(2).map_err(|_| invalida())?, byte(4).map_err(|_| invalida())?);
    let a = match hex.len() {
        6 => 255,
        8 => byte(6).map_err(|_| invalida())?,
        _ => return Err(invalida()),
    };
    Ok(format!("{r},{g},{b},{a}"))
}

fn title_xmldata(root: &Element, spec: &TitleSpec) -> Result<String, ModelError> {
    let dimensao = |nome: &str, padrao: i64| {
        profile_element(root)
            .and_then(|p| p.attributes.get(nome))
            .and_then(|v| v.parse::<i64>().ok())
            .unwrap_or(padrao)
    };
    let (w, h) = (dimensao("width", 1920), dimensao("height", 1080));
    // Caixa do tamanho da largura do quadro, com alinhamento centralizado
    // (Qt::AlignHCenter = 4): o texto fica centralizado sem precisar medir
    // a fonte. Altura aproximada pela fonte, centralizada na vertical.
    let caixa_h = ((spec.font_size as f64) * 1.4).round() as i64;
    let y = ((h - caixa_h) / 2).max(0);

    let tamanho = spec.font_size.to_string();
    let (w_txt, h_txt, caixa_txt, y_txt) = (w.to_string(), h.to_string(), caixa_h.to_string(), y.to_string());
    let mut conteudo = element_with_props(
        "content",
        &[
            ("font", &spec.font_family),
            ("font-weight", "400"),
            ("font-pixel-size", &tamanho),
            ("font-italic", "0"),
            ("font-underline", "0"),
            ("letter-spacing", "0"),
            ("box-width", &w_txt),
            ("box-height", &caixa_txt),
            ("tab-width", "80"),
            ("font-color", &spec.color_rgba),
            ("alignment", "4"),
            ("shadow", "0;#64000000;3;3;3"),
            ("typewriter", "0;2;1;0;0"),
        ],
        &[],
    );
    conteudo.children.push(XMLNode::Text(spec.text.clone()));
    let mut transform = Element::new("transform");
    transform.children.push(XMLNode::Text("1,0,0,0,1,0,0,0,1".to_string()));
    let mut posicao = element_with_props("position", &[("x", "0"), ("y", &y_txt)], &[]);
    posicao.children.push(XMLNode::Element(transform));
    let mut item = element_with_props("item", &[("type", "QGraphicsTextItem"), ("z-index", "0")], &[]);
    item.children.push(XMLNode::Element(posicao));
    item.children.push(XMLNode::Element(conteudo));

    let viewport = format!("0,0,{w},{h}");
    let mut titulo = element_with_props("kdenlivetitle", &[("width", &w_txt), ("height", &h_txt), ("LC_NUMERIC", "C")], &[]);
    titulo.children.push(XMLNode::Element(item));
    titulo.children.push(XMLNode::Element(element_with_props("startviewport", &[("rect", &viewport)], &[])));
    titulo.children.push(XMLNode::Element(element_with_props("endviewport", &[("rect", &viewport)], &[])));
    titulo.children.push(XMLNode::Element(element_with_props("background", &[("color", "0,0,0,0")], &[])));

    let mut buf = Vec::new();
    titulo
        .write_with_config(&mut buf, EmitterConfig::new().write_document_declaration(false))
        .map_err(|e| ModelError::Write(e.to_string()))?;
    String::from_utf8(buf).map_err(|e| ModelError::Write(e.to_string()))
}

/// Lê de volta texto/fonte/tamanho/cor de um título — `kdenlivetitle` (do
/// `xmldata`, inclusive títulos criados no próprio Kdenlive) ou `pango`
/// (formato antigo deste projeto). `None` se o produtor não for um título.
pub fn title_spec(producer: &Element) -> Option<TitleSpec> {
    match get_property(producer, "mlt_service").as_deref() {
        Some("kdenlivetitle") => {
            let xml = parse(&get_property(producer, "xmldata")?).ok()?;
            let conteudo = xml
                .children
                .iter()
                .filter_map(XMLNode::as_element)
                .filter(|e| attr_eq(e, "item", "type", "QGraphicsTextItem"))
                .find_map(|item| item.get_child("content"))?;
            Some(TitleSpec {
                text: conteudo.get_text().map(|t| t.into_owned()).unwrap_or_default(),
                font_family: conteudo.attributes.get("font").cloned().unwrap_or_else(|| "Sans".to_string()),
                font_size: conteudo.attributes.get("font-pixel-size").and_then(|v| v.parse().ok()).unwrap_or(48),
                color_rgba: conteudo.attributes.get("font-color").cloned().unwrap_or_else(|| "255,255,255,255".to_string()),
            })
        }
        Some("pango") => Some(TitleSpec {
            text: get_property(producer, "text").unwrap_or_default(),
            font_family: get_property(producer, "family").unwrap_or_else(|| "Sans".to_string()),
            font_size: get_property(producer, "size").and_then(|v| v.parse().ok()).unwrap_or(48),
            color_rgba: get_property(producer, "fgcolour")
                .and_then(|c| color_to_rgba(&c).ok())
                .unwrap_or_else(|| "255,255,255,255".to_string()),
        }),
        _ => None,
    }
}

/// Cria um clipe de título na bin do projeto. `color_hex` aceita `#RRGGBB`,
/// `#RRGGBBAA`, `0xRRGGBB` ou `0xRRGGBBAA`. Devolve o id do novo produtor.
#[allow(clippy::too_many_arguments)]
pub fn add_title_clip(
    root: &mut Element,
    id: &str,
    text: &str,
    duration_frames: i64,
    font_family: &str,
    font_size: i64,
    color_hex: &str,
) -> Result<String, ModelError> {
    if duration_frames <= 0 {
        return Err(ModelError::Invalid("a duração precisa ser maior que zero".to_string()));
    }
    if font_size <= 0 {
        return Err(ModelError::Invalid("o tamanho da fonte precisa ser maior que zero".to_string()));
    }
    let spec = TitleSpec {
        text: text.to_string(),
        font_family: font_family.to_string(),
        font_size,
        color_rgba: color_to_rgba(color_hex)?,
    };
    let xmldata = title_xmldata(root, &spec)?;
    let mut producer = element_with_props(
        "producer",
        &[("id", id), ("in", "0"), ("out", &(duration_frames - 1).to_string())],
        &[
            ("length", &duration_frames.to_string()),
            ("mlt_service", "kdenlivetitle"),
            ("kdenlive:clipname", text),
            ("xmldata", &xmldata),
        ],
    );
    assign_bin_identity(root, &mut producer);
    insert_bin_producer(root, producer)?;
    Ok(id.to_string())
}

/// Atualiza texto e/ou aparência de um clipe de título já existente.
/// Erro se `producer_id` não for um clipe de título.
pub fn update_title_clip(
    root: &mut Element,
    producer_id: &str,
    text: Option<&str>,
    font_family: Option<&str>,
    font_size: Option<i64>,
    color_hex: Option<&str>,
) -> Result<(), ModelError> {
    let producer = find_producer(root, producer_id)
        .ok_or_else(|| ModelError::NotFound(format!("clipe não encontrado: {producer_id}")))?;
    let mut spec = title_spec(producer)
        .ok_or_else(|| ModelError::Invalid(format!("\"{producer_id}\" não é um clipe de título")))?;
    let era_pango = get_property(producer, "mlt_service").as_deref() == Some("pango");
    if let Some(t) = text {
        spec.text = t.to_string();
    }
    if let Some(f) = font_family {
        spec.font_family = f.to_string();
    }
    if let Some(s) = font_size {
        if s <= 0 {
            return Err(ModelError::Invalid("o tamanho da fonte precisa ser maior que zero".to_string()));
        }
        spec.font_size = s;
    }
    if let Some(c) = color_hex {
        spec.color_rgba = color_to_rgba(c)?;
    }
    // Atualizar um título `pango` antigo já o converte para `kdenlivetitle`.
    let xmldata = title_xmldata(root, &spec)?;
    let producer = find_producer_mut(root, producer_id).expect("confirmado acima que existe");
    if era_pango {
        for antiga in ["text", "family", "size", "fgcolour", "bgcolour"] {
            remove_property(producer, antiga);
        }
        set_property(producer, "mlt_service", "kdenlivetitle");
    }
    set_property(producer, "xmldata", &xmldata);
    set_property(producer, "kdenlive:clipname", &spec.text);
    Ok(())
}

// ---------------------------------------------------------------------------
// Áudio: filtro `volume` padrão do MLT (nível em dB e normalização) —
// confirmado via `melt -query "filter=volume"`.
// ---------------------------------------------------------------------------

/// Define o nível (em dB) de um clipe, via o parâmetro `level` do filtro
/// `volume` (cria o filtro se ainda não existir no clipe).
pub fn audio_set_level(root: &mut Element, track_id: &str, index: usize, db: f64) -> Result<String, ModelError> {
    set_or_create_filter_param(root, track_id, index, "volume", "level", &format!("{db}dB"))
}

/// Propriedade que marca o filtro `volume` criado por `audio_duck`, para
/// que reaplicar o ducking troque só ele e deixe em paz um `volume` do
/// `audio_set_levels`/`audio_normalize` no mesmo clipe.
const PROP_DUCKING: &str = "kdenlive-mcp-plus:ducking";

/// Aplica o ducking ao clipe `track_id:index`: um filtro `volume` próprio,
/// com `level` animado pelos `keyframes` `(quadro local, dB)`. Substitui o
/// filtro de ducking anterior do clipe, se houver. Com `keyframes` vazio só
/// remove o anterior e devolve `None`; senão devolve o id do filtro.
pub fn set_ducking(
    root: &mut Element,
    track_id: &str,
    index: usize,
    keyframes: &[(i64, f64)],
) -> Result<Option<String>, ModelError> {
    let entry = get_clip_mut(root, track_id, index)
        .ok_or_else(|| ModelError::NotFound(format!("clipe não encontrado: {track_id}:{index}")))?;
    entry.children.retain(|n| {
        !matches!(n, XMLNode::Element(e) if e.name == "filter" && get_property(e, PROP_DUCKING).is_some())
    });
    if keyframes.is_empty() {
        return Ok(None);
    }
    let filter_id = add_filter(root, track_id, index, "volume")?;
    let nivel = format_keyframes(keyframes.iter().map(|&(quadro, db)| (quadro, format!("{db}"))).collect());
    set_filter_param(root, track_id, index, &filter_id, "level", &nivel)?;
    set_filter_param(root, track_id, index, &filter_id, PROP_DUCKING, "1")?;
    Ok(Some(filter_id))
}

/// Normaliza o loudness de um clipe para a amplitude alvo (em dB, ex.
/// `-12.0` para o padrão do próprio filtro `volume`), via seu parâmetro
/// `normalize`.
pub fn audio_normalize(root: &mut Element, track_id: &str, index: usize, target_db: f64) -> Result<String, ModelError> {
    set_or_create_filter_param(root, track_id, index, "volume", "normalize", &format!("{target_db}dB"))
}

// ---------------------------------------------------------------------------
// Transformação de clipe: filtro `qtblend` padrão do MLT (posição, escala,
// rotação) — confirmado via `melt -query "filter=qtblend"` e renderização.
// Este projeto só repassa os valores dados pelo agente para o parâmetro
// `rect`/`rotation` do qtblend; não calcula geometria nenhuma por conta
// própria (ver `clip_reframe_vertical`, ainda não implementada, em
// docs/04-ferramentas.md, por depender de cálculo geométrico que não dá
// para verificar visualmente neste ambiente sem tela).
// ---------------------------------------------------------------------------

#[allow(clippy::too_many_arguments)]
pub fn set_clip_transform(
    root: &mut Element,
    track_id: &str,
    index: usize,
    x: f64,
    y: f64,
    width: f64,
    height: f64,
    rotation: Option<f64>,
) -> Result<String, ModelError> {
    let filter_id = set_or_create_filter_param(
        root,
        track_id,
        index,
        "qtblend",
        "rect",
        &format!("{x} {y} {width} {height} 1"),
    )?;
    if let Some(graus) = rotation {
        set_filter_param(root, track_id, index, &filter_id, "rotation", &graus.to_string())?;
    }
    Ok(filter_id)
}

// ---------------------------------------------------------------------------
// Transições (entre duas trilhas, como `<transition>` filho do tractor
// principal — sintaxe confirmada empiricamente com `melt`)
// ---------------------------------------------------------------------------

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
    set_property(&mut transicao, "mlt_service", service);
    // Mesmo motivo do `kdenlive_id` em `add_filter`: sem essa propriedade o
    // Kdenlive real marca a transição como "faltando" e a remove ao abrir.
    set_property(&mut transicao, "kdenlive_id", service);
    set_property(&mut transicao, "a_track", &a.to_string());
    set_property(&mut transicao, "b_track", &b.to_string());
    // As trilhas foram escolhidas explicitamente pelo agente: sem isto o
    // Kdenlive "corrige" para a trilha de vídeo logo abaixo quando as duas
    // não são vizinhas (meltBuilder, `force_track == 0`).
    set_property(&mut transicao, "force_track", "1");
    tractor.children.push(XMLNode::Element(transicao));
    rebuild_internal_transitions(root);
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

/// `mlt_service` de um filtro/transição — atributo (formato antigo deste
/// projeto) ou propriedade (formato do Kdenlive).
fn service_of(e: &Element) -> Option<String> {
    get_property(e, "mlt_service").or_else(|| e.attributes.get("mlt_service").cloned())
}

/// `(id, mlt_service)` das transições criadas pelo usuário (sem as internas
/// de composição de trilha).
pub fn list_transitions(root: &Element) -> Vec<(String, String)> {
    let Some(tractor) = main_tractor(root) else {
        return Vec::new();
    };
    tractor
        .children
        .iter()
        .filter_map(XMLNode::as_element)
        .filter(|e| e.name == "transition" && !is_internal_transition(e))
        .filter_map(|e| Some((e.attributes.get("id")?.clone(), service_of(e)?)))
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
// Guias e marcadores, no formato do próprio Kdenlive (confirmado no código-
// fonte: `MarkerListModel::importFromJson`/`toJson`):
// - guias da timeline: propriedade `kdenlive:sequenceproperties.guides` da
//   sequência, JSON `[{"pos":quadro,"comment":"...","type":categoria}]`;
// - marcadores de clipe: propriedade `kdenlive:markers` do clipe da bin, mesmo
//   JSON, com `pos` em quadros da MÍDIA do clipe (0 = início do arquivo).
// O Kdenlive não tem "marcador da timeline" separado de guia, então os
// marcadores gerais deste projeto são guias da categoria `1` (azul), e as
// guias comuns, da categoria `0`; o campo extra `id` identifica cada item
// para as ferramentas (o Kdenlive ignora campos que não conhece, mas ao
// salvar pela GUI ele os descarta — nesse caso os ids são regerados).
// ---------------------------------------------------------------------------

const PROP_GUIDES: &str = "kdenlive:sequenceproperties.guides";
const PROP_CLIP_MARKERS: &str = "kdenlive:markers";
const GUIDE_CATEGORY: i64 = 0;
const TIMELINE_MARKER_CATEGORY: i64 = 1;

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
    /// Id do clipe da bin a que o marcador pertence; `None` = marcador
    /// geral da timeline.
    pub clip_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct RawMarker {
    pos: i64,
    #[serde(default)]
    comment: String,
    #[serde(default, rename = "type")]
    category: i64,
    #[serde(default, skip_serializing_if = "is_zero")]
    duration: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    id: Option<String>,
}

fn is_zero(n: &i64) -> bool {
    *n == 0
}

#[derive(Debug, Clone, PartialEq)]
enum Owner {
    Sequence,
    Clip(String),
}

fn parse_raw(texto: Option<String>) -> Vec<RawMarker> {
    texto.and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default()
}

fn is_guide(owner: &Owner, m: &RawMarker) -> bool {
    *owner == Owner::Sequence && m.category != TIMELINE_MARKER_CATEGORY
}

fn id_prefix(owner: &Owner, m: &RawMarker) -> &'static str {
    if is_guide(owner, m) { "guide" } else { "marker" }
}

/// Todos os itens (guias e marcadores), com o `id` preenchido: o gravado,
/// ou — para itens vindos do Kdenlive, que não grava ids — um novo, a
/// partir do maior número já usado com o mesmo prefixo.
fn load_all(root: &Element) -> Vec<(Owner, RawMarker)> {
    let mut itens: Vec<(Owner, RawMarker)> = Vec::new();
    if let Some(t) = main_tractor(root) {
        for m in parse_raw(get_property(t, PROP_GUIDES)) {
            itens.push((Owner::Sequence, m));
        }
    }
    for p in root.children.iter().filter_map(XMLNode::as_element).filter(|e| e.name == "producer" || e.name == "chain") {
        if let Some(pid) = p.attributes.get("id") {
            for m in parse_raw(get_property(p, PROP_CLIP_MARKERS)) {
                itens.push((Owner::Clip(pid.clone()), m));
            }
        }
    }
    let mut max_n = std::collections::HashMap::<&'static str, i64>::new();
    for (owner, m) in &itens {
        let prefixo = id_prefix(owner, m);
        if let Some(n) = m.id.as_deref().and_then(|i| i.strip_prefix(prefixo)).and_then(|n| n.parse::<i64>().ok()) {
            let atual = max_n.entry(prefixo).or_insert(-1);
            *atual = (*atual).max(n);
        }
    }
    for (owner, m) in itens.iter_mut() {
        if m.id.is_none() {
            let prefixo = id_prefix(owner, m);
            let proximo = max_n.entry(prefixo).or_insert(-1);
            *proximo += 1;
            m.id = Some(format!("{prefixo}{proximo}"));
        }
    }
    itens
}

fn next_item_id(itens: &[(Owner, RawMarker)], prefixo: &str) -> String {
    let max_n = itens
        .iter()
        .filter_map(|(_, m)| m.id.as_deref()?.strip_prefix(prefixo)?.parse::<i64>().ok())
        .max()
        .unwrap_or(-1);
    format!("{prefixo}{}", max_n + 1)
}

/// Regrava as propriedades de guias (na sequência) e de marcadores (em cada
/// clipe da bin) a partir de `itens`, removendo `kdenlive:markers` dos
/// clipes que ficaram sem marcador.
fn store_all(root: &mut Element, itens: &[(Owner, RawMarker)]) -> Result<(), ModelError> {
    let json = |lista: Vec<&RawMarker>| serde_json::to_string(&lista).map_err(|e| ModelError::Invalid(e.to_string()));
    let da_sequencia: Vec<&RawMarker> = itens.iter().filter(|(o, _)| *o == Owner::Sequence).map(|(_, m)| m).collect();
    let texto = json(da_sequencia)?;
    let tractor = main_tractor_mut(root)
        .ok_or_else(|| ModelError::NotFound("tractor da sequência não encontrado".to_string()))?;
    set_property(tractor, PROP_GUIDES, &texto);

    let ids_de_producer: Vec<String> = root
        .children
        .iter()
        .filter_map(XMLNode::as_element)
        .filter(|e| e.name == "producer" || e.name == "chain")
        .filter_map(|e| e.attributes.get("id").cloned())
        .collect();
    for pid in ids_de_producer {
        let do_clipe: Vec<&RawMarker> = itens
            .iter()
            .filter(|(o, _)| matches!(o, Owner::Clip(c) if *c == pid))
            .map(|(_, m)| m)
            .collect();
        let texto = if do_clipe.is_empty() { None } else { Some(json(do_clipe)?) };
        if let Some(p) = find_producer_mut(root, &pid) {
            match texto {
                Some(t) => set_property(p, PROP_CLIP_MARKERS, &t),
                None => remove_property(p, PROP_CLIP_MARKERS),
            }
        }
    }
    Ok(())
}

pub fn list_guides(root: &Element) -> Vec<Guide> {
    load_all(root)
        .into_iter()
        .filter(|(o, m)| is_guide(o, m))
        .map(|(_, m)| Guide { id: m.id.unwrap_or_default(), position: m.pos, comment: m.comment })
        .collect()
}

pub fn add_guide(root: &mut Element, position: i64, comment: &str) -> Result<String, ModelError> {
    let mut itens = load_all(root);
    let id = next_item_id(&itens, "guide");
    itens.push((
        Owner::Sequence,
        RawMarker { pos: position, comment: comment.to_string(), category: GUIDE_CATEGORY, duration: 0, id: Some(id.clone()) },
    ));
    store_all(root, &itens)?;
    Ok(id)
}

pub fn remove_guide(root: &mut Element, id: &str) -> Result<(), ModelError> {
    let mut itens = load_all(root);
    let antes = itens.len();
    itens.retain(|(o, m)| !(is_guide(o, m) && m.id.as_deref() == Some(id)));
    if itens.len() == antes {
        return Err(ModelError::NotFound(format!("guia não encontrada: {id}")));
    }
    store_all(root, &itens)
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
    let mut itens = load_all(root);
    let (_, guia) = itens
        .iter_mut()
        .find(|(o, m)| is_guide(o, m) && m.id.as_deref() == Some(id))
        .ok_or_else(|| ModelError::NotFound(format!("guia não encontrada: {id}")))?;
    if let Some(p) = position {
        guia.pos = p;
    }
    if let Some(c) = comment {
        guia.comment = c.to_string();
    }
    store_all(root, &itens)
}

pub fn list_markers(root: &Element) -> Vec<Marker> {
    load_all(root)
        .into_iter()
        .filter(|(o, m)| !is_guide(o, m))
        .map(|(o, m)| Marker {
            id: m.id.unwrap_or_default(),
            position: m.pos,
            comment: m.comment,
            clip_id: match o {
                Owner::Clip(c) => Some(c),
                Owner::Sequence => None,
            },
        })
        .collect()
}

/// Descobre o clipe da bin de um `clip_ref`: `"<trilha>:<índice>"` (o clipe
/// da bin que originou aquele corte da timeline — mesmo `kdenlive:id`, mesmo
/// que o corte use um produtor de velocidade) ou o id de um clipe da bin.
fn resolve_bin_clip(root: &Element, clip_ref: &str) -> Result<String, ModelError> {
    let bin = bin_clip_ids(root);
    if bin.iter().any(|id| id == clip_ref) {
        return Ok(clip_ref.to_string());
    }
    let (track_id, indice) = clip_ref
        .rsplit_once(':')
        .and_then(|(t, i)| Some((t, i.parse::<usize>().ok()?)))
        .ok_or_else(|| ModelError::NotFound(format!("clipe não encontrado: {clip_ref}")))?;
    let entry = get_clip(root, track_id, indice)
        .ok_or_else(|| ModelError::NotFound(format!("clipe não encontrado: {clip_ref}")))?;
    let produtor_id = entry.attributes.get("producer").cloned().unwrap_or_default();
    let kid = get_property(entry, "kdenlive:id")
        .or_else(|| find_producer(root, &produtor_id).and_then(|p| get_property(p, "kdenlive:id")));
    bin.into_iter()
        .find(|id| id == &produtor_id || (kid.is_some() && find_producer(root, id).and_then(|p| get_property(p, "kdenlive:id")) == kid))
        .ok_or_else(|| ModelError::NotFound(format!("o clipe {clip_ref} não tem um clipe correspondente na bin")))
}

/// Adiciona um marcador. Sem `clip_ref`: marcador geral da timeline (uma
/// guia da categoria 1 no Kdenlive), `position` em quadros da timeline. Com
/// `clip_ref` (`"<trilha>:<índice>"` ou id de clipe da bin): marcador do
/// clipe, `position` em quadros da mídia do clipe.
pub fn add_marker(
    root: &mut Element,
    position: i64,
    comment: &str,
    clip_ref: Option<&str>,
) -> Result<String, ModelError> {
    let dono = match clip_ref {
        Some(r) => Owner::Clip(resolve_bin_clip(root, r)?),
        None => Owner::Sequence,
    };
    let mut itens = load_all(root);
    let id = next_item_id(&itens, "marker");
    let categoria = if dono == Owner::Sequence { TIMELINE_MARKER_CATEGORY } else { GUIDE_CATEGORY };
    itens.push((dono, RawMarker { pos: position, comment: comment.to_string(), category: categoria, duration: 0, id: Some(id.clone()) }));
    store_all(root, &itens)?;
    Ok(id)
}

pub fn remove_marker(root: &mut Element, id: &str) -> Result<(), ModelError> {
    let mut itens = load_all(root);
    let antes = itens.len();
    itens.retain(|(o, m)| is_guide(o, m) || m.id.as_deref() != Some(id));
    if itens.len() == antes {
        return Err(ModelError::NotFound(format!("marcador não encontrado: {id}")));
    }
    store_all(root, &itens)
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
    fn documento_novo_tem_documentid_numerico_e_ensure_nao_sobrescreve() {
        let mut doc = new_document(crate::model::profile::DEFAULT_PROFILE).expect("perfil padrão válido");
        let id = get_property(main_bin(&doc).unwrap(), PROP_DOCUMENT_ID).expect("documentid presente");
        assert!(id.parse::<u64>().is_ok(), "documentid precisa ser numérico, como no Kdenlive: {id}");

        // já existe: não troca
        ensure_document_id(&mut doc);
        assert_eq!(get_property(main_bin(&doc).unwrap(), PROP_DOCUMENT_ID), Some(id));

        // ausente (arquivo de uma versão antiga): é criado
        remove_property(main_bin_mut(&mut doc).unwrap(), PROP_DOCUMENT_ID);
        assert!(get_property(main_bin(&doc).unwrap(), PROP_DOCUMENT_ID).is_none());
        ensure_document_id(&mut doc);
        assert!(get_property(main_bin(&doc).unwrap(), PROP_DOCUMENT_ID).is_some());
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
        assert_eq!(track_length(find_track_playlist(&doc, &track_id).unwrap()), 50);

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
        let clip = add_bin_clip(&mut doc, "producer0", "/tmp/a.mp4", "a", 0, 99).expect("importa");
        let track_id = add_track(&mut doc, "track0", TrackKind::Video).expect("trilha");
        add_clip_to_track(&mut doc, &track_id, &clip, 10, 59, 0).expect("posiciona");

        let id_timeline = add_marker(&mut doc, 50, "marcador geral", None).expect("adiciona marcador de timeline");
        let id_clipe = add_marker(&mut doc, 10, "marcador do clipe", Some("track0:0")).expect("adiciona marcador de clipe");
        let guia = add_guide(&mut doc, 7, "uma guia").expect("guia");

        let lista = list_markers(&doc);
        assert_eq!(lista.len(), 2, "guias não entram na lista de marcadores");
        assert!(lista.iter().any(|m| m.id == id_timeline && m.clip_id.is_none()));
        assert!(lista.iter().any(|m| m.id == id_clipe && m.clip_id.as_deref() == Some("producer0")));
        assert_eq!(list_guides(&doc).len(), 1, "marcador geral não entra na lista de guias");

        // Formato nativo do Kdenlive: o marcador do clipe vai no produtor da bin,
        // o geral e a guia na sequência.
        let bruto = get_property(find_producer(&doc, "producer0").unwrap(), "kdenlive:markers").expect("kdenlive:markers");
        assert!(bruto.contains("\"pos\":10") && bruto.contains("\"comment\":\"marcador do clipe\""), "{bruto}");
        let seq = get_property(main_tractor(&doc).unwrap(), "kdenlive:sequenceproperties.guides").expect("guides");
        assert!(seq.contains("\"pos\":7") && seq.contains("\"pos\":50"), "{seq}");

        // clip_ref inválido ou sem clipe correspondente
        assert!(matches!(add_marker(&mut doc, 1, "x", Some("track0:9")), Err(ModelError::NotFound(_))));
        assert!(matches!(add_marker(&mut doc, 1, "x", Some("naoexiste")), Err(ModelError::NotFound(_))));

        remove_marker(&mut doc, &id_timeline).expect("remove");
        remove_marker(&mut doc, &id_clipe).expect("remove o do clipe");
        assert!(list_markers(&doc).is_empty());
        assert!(
            get_property(find_producer(&doc, "producer0").unwrap(), "kdenlive:markers").is_none(),
            "clipe sem marcadores não deve manter a propriedade"
        );
        assert!(get_guide(&doc, &guia).is_some(), "remover marcador não pode apagar guia");
    }

    #[test]
    fn guias_vindas_do_kdenlive_sem_id_ganham_ids_estaveis() {
        let mut doc = new_document(DEFAULT_PROFILE_PARA_TESTE).expect("perfil válido");
        let seq = main_tractor_mut(&mut doc).unwrap();
        set_property(
            seq,
            "kdenlive:sequenceproperties.guides",
            r#"[{"pos":25,"comment":"a","type":0},{"pos":50,"comment":"b","type":3},{"pos":75,"comment":"c","type":1}]"#,
        );
        let guias = list_guides(&doc);
        assert_eq!(guias.iter().map(|g| g.position).collect::<Vec<_>>(), vec![25, 50]);
        assert_eq!(list_markers(&doc).len(), 1, "categoria 1 é marcador geral");

        let nova = add_guide(&mut doc, 99, "d").expect("adiciona");
        let ids: Vec<String> = list_guides(&doc).into_iter().map(|g| g.id).collect();
        assert_eq!(ids.len(), 3);
        assert!(ids.contains(&nova));
        let unicos: std::collections::HashSet<_> = ids.iter().collect();
        assert_eq!(unicos.len(), 3, "ids repetidos: {ids:?}");
        let bruto = get_property(main_tractor(&doc).unwrap(), "kdenlive:sequenceproperties.guides").unwrap();
        assert!(bruto.contains("\"type\":3"), "categoria original deve ser preservada: {bruto}");
    }

    #[test]
    fn cor_e_nota_do_clipe_usam_as_propriedades_do_kdenlive() {
        let mut doc = new_document(DEFAULT_PROFILE_PARA_TESTE).expect("perfil válido");
        add_bin_clip(&mut doc, "producer0", "/tmp/a.mp4", "a", 0, 9).expect("importa");
        bin_clip_set_color(&mut doc, "producer0", "#FF0000").expect("cor");
        bin_clip_set_note(&mut doc, "producer0", "uma nota").expect("nota");
        let p = find_producer(&doc, "producer0").unwrap();
        assert_eq!(get_property(p, "kdenlive:tags"), Some("#ff0000".to_string()));
        assert_eq!(get_property(p, "kdenlive:description"), Some("uma nota".to_string()));
        assert_eq!(bin_clip_color_note(p), (Some("#ff0000".to_string()), Some("uma nota".to_string())));
        assert!(matches!(bin_clip_set_color(&mut doc, "producer0", "vermelho"), Err(ModelError::Invalid(_))));
    }

    #[test]
    fn pastas_da_bin_e_mover_clipe_entre_elas() {
        let mut doc = new_document(DEFAULT_PROFILE_PARA_TESTE).expect("perfil válido");
        let producer_id = add_bin_clip(&mut doc, "producer0", "/tmp/a.mp4", "a.mp4", 0, 9).expect("importa");

        assert!(list_bin_folders(&doc).is_empty());
        let raiz = create_bin_folder(&mut doc, None, "Vídeos").expect("cria pasta raiz");
        let sub = create_bin_folder(&mut doc, Some(&raiz), "Brutos").expect("cria subpasta");

        let pastas = list_bin_folders(&doc);
        assert_eq!(pastas.len(), 2);
        assert!(pastas.iter().any(|(pai, id, nome)| pai == "-1" && id == &raiz && nome == "Vídeos"));
        assert!(pastas.iter().any(|(pai, id, nome)| pai == &raiz && id == &sub && nome == "Brutos"));

        bin_move_clip(&mut doc, &producer_id, Some(&sub)).expect("move o clipe");
        assert_eq!(
            get_property(find_producer(&doc, &producer_id).unwrap(), "kdenlive:folderid"),
            Some(sub)
        );

        bin_move_clip(&mut doc, &producer_id, None).expect("move de volta pra raiz");
        assert_eq!(get_property(find_producer(&doc, &producer_id).unwrap(), "kdenlive:folderid"), None);
    }

    #[test]
    fn renomear_clipe_da_bin() {
        let mut doc = new_document(DEFAULT_PROFILE_PARA_TESTE).expect("perfil válido");
        let producer_id = add_bin_clip(&mut doc, "producer0", "/tmp/a.mp4", "a.mp4", 0, 9).expect("importa");
        bin_clip_rename(&mut doc, &producer_id, "novo nome").expect("renomeia");
        assert_eq!(
            get_property(find_producer(&doc, &producer_id).unwrap(), "kdenlive:clipname"),
            Some("novo nome".to_string())
        );
    }

    #[test]
    fn trocar_perfil_do_projeto() {
        let mut doc = new_document("atsc_720p_25").expect("perfil válido");
        assert_eq!(profile_element(&doc).unwrap().attributes.get("width"), Some(&"1280".to_string()));

        set_profile(&mut doc, "uhd_2160p_30").expect("troca pra 4k");
        assert_eq!(profile_element(&doc).unwrap().attributes.get("width"), Some(&"3840".to_string()));
        assert_eq!(profile_element(&doc).unwrap().attributes.get("height"), Some(&"2160".to_string()));

        let erro = set_profile(&mut doc, "perfil-que-nao-existe").expect_err("deveria falhar");
        assert!(matches!(erro, ModelError::Invalid(_)));
    }

    #[test]
    fn inserir_exatamente_no_inicio_de_um_clipe_empurra_o_resto_para_depois() {
        let mut doc = new_document(DEFAULT_PROFILE_PARA_TESTE).expect("perfil válido");
        let p0 = add_bin_clip(&mut doc, "producer0", "/tmp/a.mp4", "a", 0, 999).expect("importa a");
        let p1 = add_bin_clip(&mut doc, "producer1", "/tmp/b.mp4", "b", 0, 999).expect("importa b");
        let track_id = add_track(&mut doc, "playlist0", TrackKind::Video).expect("cria trilha");

        add_clip_to_track(&mut doc, &track_id, &p0, 0, 49, 0).expect("primeiro clipe, 50 quadros");
        // insere bem no início (posição 0) — deve empurrar o primeiro clipe para depois
        let indice = add_clip_to_track(&mut doc, &track_id, &p1, 0, 9, 0).expect("insere no início, 10 quadros");
        assert_eq!(indice, 0, "o novo clipe deve ficar no índice 0");

        let clipes = list_clips(&doc, &track_id);
        assert_eq!(clipes, vec![(0, p1.clone(), 0, 9), (1, p0.clone(), 0, 49)]);
        assert_eq!(project_duration(&doc), 60);
    }

    #[test]
    fn inserir_dentro_de_um_blank_divide_o_blank_em_dois() {
        let mut doc = new_document(DEFAULT_PROFILE_PARA_TESTE).expect("perfil válido");
        let p0 = add_bin_clip(&mut doc, "producer0", "/tmp/a.mp4", "a", 0, 999).expect("importa");
        let track_id = add_track(&mut doc, "playlist0", TrackKind::Video).expect("cria trilha");

        // cria um blank de 100 quadros (nenhum clipe na posição 0..99), com um clipe real depois
        add_clip_to_track(&mut doc, &track_id, &p0, 0, 9, 100).expect("clipe depois de um gap de 100");
        assert_eq!(track_length(find_track_playlist(&doc, &track_id).unwrap()), 110);

        // insere outro clipe de 20 quadros no meio do gap (posição 30)
        let indice = add_clip_to_track(&mut doc, &track_id, &p0, 0, 19, 30).expect("insere no meio do gap");
        assert_eq!(indice, 1, "deveria ficar entre o blank-antes e o blank-depois");
        assert_eq!(project_duration(&doc), 130, "o comprimento total não deveria mudar ao preencher um gap");
    }

    #[test]
    fn mover_clipe_para_outra_posicao_na_mesma_trilha() {
        let mut doc = new_document(DEFAULT_PROFILE_PARA_TESTE).expect("perfil válido");
        let p0 = add_bin_clip(&mut doc, "producer0", "/tmp/a.mp4", "a", 0, 999).expect("importa");
        let track_id = add_track(&mut doc, "playlist0", TrackKind::Video).expect("cria trilha");

        add_clip_to_track(&mut doc, &track_id, &p0, 0, 9, 0).expect("primeiro clipe (0..9)");
        add_clip_to_track(&mut doc, &track_id, &p0, 0, 9, 10).expect("segundo clipe (10..19)");

        // move o segundo clipe (índice 1) para o início
        let novo_indice = move_clip(&mut doc, &track_id, 1, 0).expect("move para o início");
        assert_eq!(novo_indice, 0);
        assert_eq!(list_clips(&doc, &track_id).len(), 2, "ainda devem existir 2 clipes, nenhum duplicado ou perdido");
    }

    #[test]
    fn aparar_um_clipe_altera_seu_comprimento_e_desloca_o_resto() {
        let mut doc = new_document(DEFAULT_PROFILE_PARA_TESTE).expect("perfil válido");
        let p0 = add_bin_clip(&mut doc, "producer0", "/tmp/a.mp4", "a", 0, 999).expect("importa");
        let track_id = add_track(&mut doc, "playlist0", TrackKind::Video).expect("cria trilha");

        add_clip_to_track(&mut doc, &track_id, &p0, 0, 49, 0).expect("clipe de 50 quadros");
        add_clip_to_track(&mut doc, &track_id, &p0, 0, 9, 50).expect("segundo clipe logo depois");
        assert_eq!(project_duration(&doc), 60);

        trim_clip(&mut doc, &track_id, 0, 0, 29).expect("encurta o primeiro clipe para 30 quadros");
        assert_eq!(project_duration(&doc), 40, "o segundo clipe deveria ter sido empurrado pra mais perto");

        let erro = trim_clip(&mut doc, &track_id, 0, 10, 5).expect_err("out menor que in deveria falhar");
        assert!(matches!(erro, ModelError::Invalid(_)));
    }

    #[test]
    fn dividir_um_clipe_em_dois_pedacos() {
        let mut doc = new_document(DEFAULT_PROFILE_PARA_TESTE).expect("perfil válido");
        let p0 = add_bin_clip(&mut doc, "producer0", "/tmp/a.mp4", "a", 0, 999).expect("importa");
        let track_id = add_track(&mut doc, "playlist0", TrackKind::Video).expect("cria trilha");
        add_clip_to_track(&mut doc, &track_id, &p0, 0, 99, 0).expect("clipe de 100 quadros");

        let (primeiro, segundo) = split_clip(&mut doc, &track_id, 0, 40).expect("divide em 40");
        assert_eq!(primeiro, 0);
        assert_eq!(segundo, 1);

        let clipes = list_clips(&doc, &track_id);
        assert_eq!(clipes, vec![(0, p0.clone(), 0, 39), (1, p0.clone(), 40, 99)]);
        assert_eq!(project_duration(&doc), 100, "a duração total não deveria mudar ao dividir");

        let erro = split_clip(&mut doc, &track_id, 0, 0).expect_err("split_at == in deveria falhar");
        assert!(matches!(erro, ModelError::Invalid(_)));
        let erro = split_clip(&mut doc, &track_id, 1, 999).expect_err("split_at fora do range deveria falhar");
        assert!(matches!(erro, ModelError::Invalid(_)));
    }

    #[test]
    fn titulo_cria_le_e_atualiza() {
        let mut doc = new_document(DEFAULT_PROFILE_PARA_TESTE).expect("perfil válido");
        let id = add_title_clip(&mut doc, "producer0", "Olá & <tchau>", 50, "Sans", 48, "0xffffffff").expect("cria título");

        let p = find_producer(&doc, &id).unwrap();
        assert_eq!(get_property(p, "mlt_service"), Some("kdenlivetitle".to_string()));
        assert_eq!(p.attributes.get("out"), Some(&"49".to_string()));
        let spec = title_spec(p).expect("é um título");
        assert_eq!(spec.text, "Olá & <tchau>", "texto com caracteres especiais deve sobreviver ao xmldata");
        assert_eq!(spec.color_rgba, "255,255,255,255");
        assert!(bin_clip_ids(&doc).contains(&id), "título deveria entrar na bin, como qualquer clipe");

        update_title_clip(&mut doc, &id, Some("Novo texto"), None, Some(72), Some("#ff000080")).expect("atualiza");
        let spec = title_spec(find_producer(&doc, &id).unwrap()).unwrap();
        assert_eq!(spec.text, "Novo texto");
        assert_eq!(spec.font_size, 72);
        assert_eq!(spec.color_rgba, "255,0,0,128");
        assert_eq!(spec.font_family, "Sans", "fonte não informada deve ser mantida");

        let erro = update_title_clip(&mut doc, &id, None, None, None, Some("azul")).expect_err("cor inválida");
        assert!(matches!(erro, ModelError::Invalid(_)));

        // id que não é um título deve falhar
        let outro = add_bin_clip(&mut doc, "producer1", "/tmp/a.mp4", "a", 0, 9).expect("importa outro clipe");
        let erro = update_title_clip(&mut doc, &outro, Some("x"), None, None, None).expect_err("não é título");
        assert!(matches!(erro, ModelError::Invalid(_)));
    }

    #[test]
    fn keyframes_sao_definidos_listados_e_removidos_em_ordem() {
        let mut doc = new_document(DEFAULT_PROFILE_PARA_TESTE).expect("perfil válido");
        let producer_id = add_bin_clip(&mut doc, "producer0", "/tmp/a.mp4", "a", 0, 99).expect("importa");
        let track_id = add_track(&mut doc, "playlist0", TrackKind::Video).expect("cria trilha");
        add_clip_to_track(&mut doc, &track_id, &producer_id, 0, 99, 0).expect("posiciona");
        let filter_id = add_filter(&mut doc, &track_id, 0, "brightness").expect("adiciona filtro");

        assert!(keyframe_list(&doc, &track_id, 0, &filter_id, "level").is_empty());

        keyframe_set(&mut doc, &track_id, 0, &filter_id, "level", 50, "1.0").expect("define keyframe");
        keyframe_set(&mut doc, &track_id, 0, &filter_id, "level", 0, "0.0").expect("define outro, fora de ordem");
        keyframe_set(&mut doc, &track_id, 0, &filter_id, "level", 99, "0.0").expect("e outro");

        let lista = keyframe_list(&doc, &track_id, 0, &filter_id, "level");
        assert_eq!(
            lista,
            vec![(0, "0.0".to_string()), (50, "1.0".to_string()), (99, "0.0".to_string())],
            "deveriam vir ordenados por quadro, mesmo tendo sido definidos fora de ordem"
        );

        // redefinir o mesmo quadro substitui, não duplica
        keyframe_set(&mut doc, &track_id, 0, &filter_id, "level", 50, "0.5").expect("redefine");
        assert_eq!(keyframe_list(&doc, &track_id, 0, &filter_id, "level").len(), 3);
        assert!(keyframe_list(&doc, &track_id, 0, &filter_id, "level").contains(&(50, "0.5".to_string())));

        keyframe_remove(&mut doc, &track_id, 0, &filter_id, "level", 0).expect("remove");
        assert_eq!(keyframe_list(&doc, &track_id, 0, &filter_id, "level").len(), 2);

        let erro = keyframe_remove(&mut doc, &track_id, 0, &filter_id, "level", 999).expect_err("não existe");
        assert!(matches!(erro, ModelError::NotFound(_)));
    }

    #[test]
    fn audio_set_level_e_normalize_usam_o_filtro_volume() {
        let mut doc = new_document(DEFAULT_PROFILE_PARA_TESTE).expect("perfil válido");
        let producer_id = add_bin_clip(&mut doc, "producer0", "/tmp/a.mp4", "a", 0, 99).expect("importa");
        let track_id = add_track(&mut doc, "playlist0", TrackKind::Audio).expect("cria trilha");
        add_clip_to_track(&mut doc, &track_id, &producer_id, 0, 99, 0).expect("posiciona");

        let filter_id = audio_set_level(&mut doc, &track_id, 0, -6.0).expect("define nível");
        assert_eq!(get_filter_param(&doc, &track_id, 0, &filter_id, "level"), Some("-6dB".to_string()));

        // chamar de novo não deve criar um segundo filtro "volume"
        audio_normalize(&mut doc, &track_id, 0, -12.0).expect("normaliza");
        assert_eq!(list_filters(&doc, &track_id, 0).iter().filter(|(_, s)| s == "volume").count(), 1);
    }

    #[test]
    fn set_clip_transform_repassa_rect_e_rotacao_para_o_qtblend() {
        let mut doc = new_document(DEFAULT_PROFILE_PARA_TESTE).expect("perfil válido");
        let producer_id = add_bin_clip(&mut doc, "producer0", "/tmp/a.mp4", "a", 0, 99).expect("importa");
        let track_id = add_track(&mut doc, "playlist0", TrackKind::Video).expect("cria trilha");
        add_clip_to_track(&mut doc, &track_id, &producer_id, 0, 99, 0).expect("posiciona");

        let filter_id = set_clip_transform(&mut doc, &track_id, 0, 10.0, 20.0, 640.0, 360.0, Some(45.0)).expect("aplica transform");
        assert_eq!(get_filter_param(&doc, &track_id, 0, &filter_id, "rect"), Some("10 20 640 360 1".to_string()));
        assert_eq!(get_filter_param(&doc, &track_id, 0, &filter_id, "rotation"), Some("45".to_string()));
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
