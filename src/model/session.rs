//! A sessão de edição: o documento MLT XML atual, mais histórico de
//! undo/redo por snapshot (até 50 entradas, conforme pedido no briefing
//! original) e o caminho de onde foi aberto/para onde foi salvo.

use std::path::{Path, PathBuf};

use xmltree::Element;

use super::document::{self, ModelError};
pub use super::document::TrackKind;

const MAX_HISTORY: usize = 50;

pub struct Session {
    pub doc: Element,
    pub path: Option<PathBuf>,
    pub modified: bool,
    /// Pilha de `(estado antes da operação, descrição da operação)`.
    history: Vec<(Element, String)>,
    /// Pilha de `(estado depois da operação, descrição da operação)`,
    /// usada para refazer.
    redo_stack: Vec<(Element, String)>,
    /// Próximo número livre para `producerN`/`playlistN`. Inicializado uma
    /// única vez a partir do documento (na criação/abertura da sessão) e só
    /// cresce a partir daí — nunca reaproveita o id de algo removido
    /// durante a mesma sessão, mesmo que o documento volte a ficar "livre"
    /// naquele número.
    next_producer_n: u64,
    next_playlist_n: u64,
}

impl Session {
    pub fn new(profile_name: &str) -> Result<Self, ModelError> {
        let doc = document::new_document(profile_name)?;
        Ok(Self::from_doc(doc, None))
    }

    pub fn open(path: &Path) -> Result<Self, ModelError> {
        let text = std::fs::read_to_string(path).map_err(|e| ModelError::Io(e.to_string()))?;
        let doc = document::parse(&text)?;
        Ok(Self::from_doc(doc, Some(path.to_path_buf())))
    }

    fn from_doc(doc: Element, path: Option<PathBuf>) -> Self {
        let next_producer_n = document::next_available_n(&doc, "producer", "producer");
        let next_playlist_n = document::next_available_n(&doc, "playlist", "playlist");
        Self {
            doc,
            path,
            modified: false,
            history: Vec::new(),
            redo_stack: Vec::new(),
            next_producer_n,
            next_playlist_n,
        }
    }

    /// Aloca o próximo id de produtor livre para esta sessão (ex.
    /// `"producer3"`). Nunca reaproveita um número já usado nesta sessão,
    /// mesmo que esse produtor tenha sido removido.
    pub fn alloc_producer_id(&mut self) -> String {
        let n = self.next_producer_n;
        self.next_producer_n += 1;
        format!("producer{n}")
    }

    /// Aloca o próximo id de trilha (playlist) livre para esta sessão (ex.
    /// `"playlist2"`).
    pub fn alloc_playlist_id(&mut self) -> String {
        let n = self.next_playlist_n;
        self.next_playlist_n += 1;
        format!("playlist{n}")
    }

    /// Salva em `path`, ou no caminho de origem se `path` for `None`.
    pub fn save(&mut self, path: Option<&Path>) -> Result<PathBuf, ModelError> {
        let target = path
            .map(Path::to_path_buf)
            .or_else(|| self.path.clone())
            .ok_or_else(|| {
                ModelError::Invalid(
                    "este projeto ainda não tem um caminho; informe output_path".to_string(),
                )
            })?;
        let xml = document::to_string_pretty(&self.doc)?;
        std::fs::write(&target, xml).map_err(|e| ModelError::Io(e.to_string()))?;
        self.path = Some(target.clone());
        self.modified = false;
        Ok(target)
    }

    /// Registra o estado atual no histórico de undo, com uma descrição
    /// curta, **antes** de aplicar uma mutação. Limpa a pilha de redo (uma
    /// nova mutação invalida qualquer "refazer" pendente).
    pub fn snapshot_before(&mut self, description: impl Into<String>) {
        self.history.push((self.doc.clone(), description.into()));
        if self.history.len() > MAX_HISTORY {
            self.history.remove(0);
        }
        self.redo_stack.clear();
    }

    /// Chame depois de uma mutação ter sido aplicada com sucesso.
    pub fn mark_modified(&mut self) {
        self.modified = true;
    }

    /// Desfaz até `steps` operações. Devolve quantas de fato foram desfeitas.
    pub fn undo(&mut self, steps: usize) -> Result<usize, ModelError> {
        let steps = steps.min(self.history.len());
        if steps == 0 {
            return Err(ModelError::Invalid("nada para desfazer".to_string()));
        }
        for _ in 0..steps {
            let (pre_state, description) = self.history.pop().expect("checado acima");
            let post_state = std::mem::replace(&mut self.doc, pre_state);
            self.redo_stack.push((post_state, description));
        }
        self.modified = true;
        Ok(steps)
    }

    /// Refaz até `steps` operações previamente desfeitas.
    pub fn redo(&mut self, steps: usize) -> Result<usize, ModelError> {
        let steps = steps.min(self.redo_stack.len());
        if steps == 0 {
            return Err(ModelError::Invalid("nada para refazer".to_string()));
        }
        for _ in 0..steps {
            let (post_state, description) = self.redo_stack.pop().expect("checado acima");
            let pre_state = std::mem::replace(&mut self.doc, post_state);
            self.history.push((pre_state, description));
        }
        self.modified = true;
        Ok(steps)
    }

    pub fn history_len(&self) -> usize {
        self.history.len()
    }

    pub fn redo_len(&self) -> usize {
        self.redo_stack.len()
    }

    /// Descrições do histórico, da mais antiga para a mais recente.
    pub fn history_descriptions(&self) -> Vec<String> {
        self.history.iter().map(|(_, d)| d.clone()).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::document;
    use crate::model::profile::DEFAULT_PROFILE;

    #[test]
    fn ids_alocados_pela_sessao_nunca_colidem_mesmo_apos_remover() {
        let mut session = Session::new(DEFAULT_PROFILE).expect("perfil padrão válido");
        let id0 = session.alloc_producer_id();
        document::add_bin_clip(&mut session.doc, &id0, "/tmp/a.mp4", "a", 0, 9).expect("importa");
        let id1 = session.alloc_producer_id();
        document::add_bin_clip(&mut session.doc, &id1, "/tmp/b.mp4", "b", 0, 9).expect("importa");

        document::remove_bin_clip(&mut session.doc, &id1).expect("remove o segundo");

        let id2 = session.alloc_producer_id();
        document::add_bin_clip(&mut session.doc, &id2, "/tmp/c.mp4", "c", 0, 9).expect("importa outro");

        assert_ne!(id0, id2);
        assert_ne!(id1, id2, "não deveria reaproveitar o id recém-liberado dentro da mesma sessão");
    }

    #[test]
    fn sessao_aberta_de_um_arquivo_continua_a_numeracao_existente() {
        let mut original = Session::new(DEFAULT_PROFILE).expect("perfil válido");
        let id0 = original.alloc_producer_id();
        document::add_bin_clip(&mut original.doc, &id0, "/tmp/a.mp4", "a", 0, 9).expect("importa");

        let caminho = std::env::temp_dir().join(format!(
            "kdenlive_mcp_plus_session_test_{}.xml",
            std::process::id()
        ));
        original.save(Some(&caminho)).expect("salva");

        let mut reaberta = Session::open(&caminho).expect("abre de novo");
        let novo_id = reaberta.alloc_producer_id();
        assert_eq!(novo_id, "producer1", "deveria continuar depois do producer0 já existente no arquivo");

        let _ = std::fs::remove_file(&caminho);
    }

    #[test]
    fn undo_redo_vao_e_voltam_pelo_historico() {
        let mut session = Session::new(DEFAULT_PROFILE).expect("perfil válido");

        session.snapshot_before("importar a.mp4");
        let id0 = session.alloc_producer_id();
        document::add_bin_clip(&mut session.doc, &id0, "/tmp/a.mp4", "a", 0, 9).expect("importa");
        session.mark_modified();

        assert_eq!(document::bin_clip_ids(&session.doc), vec![id0.clone()]);
        assert_eq!(session.history_len(), 1);

        session.undo(1).expect("desfaz a importação");
        assert!(document::bin_clip_ids(&session.doc).is_empty());
        assert_eq!(session.history_len(), 0);
        assert_eq!(session.redo_len(), 1);

        session.redo(1).expect("refaz a importação");
        assert_eq!(document::bin_clip_ids(&session.doc), vec![id0.clone()]);
        assert_eq!(session.history_len(), 1);
        assert_eq!(session.redo_len(), 0);

        let erro = session.redo(1).expect_err("não há mais nada para refazer");
        assert!(matches!(erro, ModelError::Invalid(_)));
    }

    #[test]
    fn uma_nova_mutacao_depois_de_desfazer_limpa_o_redo() {
        let mut session = Session::new(DEFAULT_PROFILE).expect("perfil válido");

        session.snapshot_before("importar a.mp4");
        let id0 = session.alloc_producer_id();
        document::add_bin_clip(&mut session.doc, &id0, "/tmp/a.mp4", "a", 0, 9).expect("importa");

        session.undo(1).expect("desfaz");
        assert_eq!(session.redo_len(), 1);

        session.snapshot_before("importar b.mp4");
        let id1 = session.alloc_producer_id();
        document::add_bin_clip(&mut session.doc, &id1, "/tmp/b.mp4", "b", 0, 9).expect("importa outro");

        assert_eq!(session.redo_len(), 0, "uma mutação nova deve invalidar o redo pendente");
    }
}
