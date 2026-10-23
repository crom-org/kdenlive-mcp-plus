//! Aritmética do ducking de áudio (`audio_duck`, D-047): abaixar uma
//! trilha (a música) enquanto outra (a voz) está falando.
//!
//! O ganho é uma função linear por partes dos quadros da timeline: 0 dB
//! longe da fala, `-duck_db` durante a fala, com uma rampa de descida
//! (`attack`) antes de ela começar e uma de subida (`release`) depois de ela
//! acabar. O resultado vira keyframes do parâmetro `level` do filtro `volume`
//! do MLT (animado, em dB). Só aritmética: o efeito é conferido de verdade
//! medindo o áudio renderizado (`tests/mcp_stdio.rs`).

/// Parâmetros do ducking. `attack` e `release` são em quadros e valem no
/// mínimo 1 (a rampa nunca é instantânea, para não gerar dois keyframes no
/// mesmo quadro).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Parametros {
    /// Quanto abaixar, em dB (positivo: 12 = -12 dB).
    pub duck_db: f64,
    pub attack: i64,
    pub release: i64,
}

impl Parametros {
    pub fn novo(duck_db: f64, attack: i64, release: i64) -> Self {
        Self { duck_db: duck_db.abs(), attack: attack.max(1), release: release.max(1) }
    }
}

/// Funde os trechos de fala `[início, fim)` (em quadros da timeline) cujas
/// rampas se encostariam: dois trechos separados por menos que
/// `release + attack` quadros viram um só (senão o volume subiria e desceria
/// no meio de uma pausa curta). Descarta trechos vazios.
pub fn unir_trechos(mut voz: Vec<(i64, i64)>, p: &Parametros) -> Vec<(i64, i64)> {
    voz.retain(|(ini, fim)| fim > ini);
    voz.sort_unstable();
    let mut unidos: Vec<(i64, i64)> = Vec::new();
    for (ini, fim) in voz {
        match unidos.last_mut() {
            Some(ultimo) if ini - p.attack <= ultimo.1 + p.release => ultimo.1 = ultimo.1.max(fim),
            _ => unidos.push((ini, fim)),
        }
    }
    unidos
}

/// Ganho, em dB (0 ou negativo), no quadro `quadro` da timeline.
pub fn ganho(quadro: i64, trechos: &[(i64, i64)], p: &Parametros) -> f64 {
    let mut menor = 0.0_f64;
    for &(ini, fim) in trechos {
        let g = if quadro <= ini - p.attack || quadro >= fim + p.release {
            0.0
        } else if quadro < ini {
            -p.duck_db * (quadro - (ini - p.attack)) as f64 / p.attack as f64
        } else if quadro <= fim {
            -p.duck_db
        } else {
            -p.duck_db * (1.0 - (quadro - fim) as f64 / p.release as f64)
        };
        menor = menor.min(g);
    }
    menor
}

/// Keyframes `(quadro local, dB)` para um clipe que começa no quadro `pos`
/// da timeline e dura `len` quadros. O quadro local 0 é o primeiro do clipe
/// (é assim que o MLT conta a posição de um filtro anexado a um clipe).
/// Devolve vazio se o clipe não é afetado por nenhum trecho (ducking sem efeito).
pub fn keyframes(pos: i64, len: i64, trechos: &[(i64, i64)], p: &Parametros) -> Vec<(i64, f64)> {
    if len <= 0 {
        return Vec::new();
    }
    let ultimo = len - 1;
    let mut quadros: Vec<i64> = vec![0, ultimo];
    for &(ini, fim) in trechos {
        for ponto in [ini - p.attack, ini, fim, fim + p.release] {
            let local = ponto - pos;
            if (0..=ultimo).contains(&local) {
                quadros.push(local);
            }
        }
    }
    quadros.sort_unstable();
    quadros.dedup();
    let pontos: Vec<(i64, f64)> = quadros
        .into_iter()
        .map(|local| (local, (ganho(pos + local, trechos, p) * 100.0).round() / 100.0))
        .map(|(local, db)| (local, if db == 0.0 { 0.0 } else { db })) // sem "-0"
        .collect();
    if pontos.iter().all(|&(_, db)| db == 0.0) {
        return Vec::new();
    }
    pontos
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p() -> Parametros {
        Parametros::novo(12.0, 10, 20)
    }

    #[test]
    fn ganho_tem_rampa_de_descida_plateau_e_rampa_de_subida() {
        let trechos = [(100, 200)];
        assert_eq!(ganho(50, &trechos, &p()), 0.0);
        assert_eq!(ganho(90, &trechos, &p()), 0.0); // começo da rampa
        assert_eq!(ganho(95, &trechos, &p()), -6.0); // meio da descida
        assert_eq!(ganho(100, &trechos, &p()), -12.0);
        assert_eq!(ganho(150, &trechos, &p()), -12.0);
        assert_eq!(ganho(200, &trechos, &p()), -12.0);
        assert_eq!(ganho(210, &trechos, &p()), -6.0); // meio da subida (release 20)
        assert_eq!(ganho(220, &trechos, &p()), 0.0);
        assert_eq!(ganho(500, &trechos, &p()), 0.0);
    }

    #[test]
    fn trechos_proximos_sao_unidos_e_distantes_nao() {
        // pausa de 25 quadros: rampas (10 + 20 = 30) se encostam -> une
        let unidos = unir_trechos(vec![(100, 200), (225, 300)], &p());
        assert_eq!(unidos, vec![(100, 300)]);
        // pausa de 100: continuam separados
        let separados = unir_trechos(vec![(100, 200), (300, 400)], &p());
        assert_eq!(separados, vec![(100, 200), (300, 400)]);
        // fora de ordem, vazios e sobrepostos
        let bagunca = unir_trechos(vec![(300, 400), (50, 50), (100, 350)], &p());
        assert_eq!(bagunca, vec![(100, 400)]);
    }

    #[test]
    fn keyframes_do_clipe_usam_quadros_locais_e_incluem_as_pontas() {
        // clipe de 0 a 399 na timeline, fala de 100 a 200
        let k = keyframes(0, 400, &[(100, 200)], &p());
        assert_eq!(k, vec![(0, 0.0), (90, 0.0), (100, -12.0), (200, -12.0), (220, 0.0), (399, 0.0)]);
        // o mesmo clipe começando em 50: tudo desloca 50 quadros
        let k = keyframes(50, 350, &[(100, 200)], &p());
        assert_eq!(k, vec![(0, 0.0), (40, 0.0), (50, -12.0), (150, -12.0), (170, 0.0), (349, 0.0)]);
    }

    #[test]
    fn clipe_que_comeca_no_meio_da_fala_ja_comeca_abaixado() {
        let k = keyframes(150, 200, &[(100, 200)], &p());
        assert_eq!(k.first(), Some(&(0, -12.0)));
        assert_eq!(k.last(), Some(&(199, 0.0)));
    }

    #[test]
    fn clipe_fora_da_fala_nao_tem_keyframes() {
        assert!(keyframes(1000, 100, &[(100, 200)], &p()).is_empty());
        assert!(keyframes(0, 100, &[], &p()).is_empty());
        assert!(keyframes(0, 0, &[(0, 10)], &p()).is_empty());
    }

    #[test]
    fn rampa_minima_e_um_quadro() {
        let p = Parametros::novo(-6.0, 0, 0);
        assert_eq!(p.duck_db, 6.0);
        assert_eq!((p.attack, p.release), (1, 1));
        let k = keyframes(0, 100, &[(10, 20)], &p);
        assert_eq!(k, vec![(0, 0.0), (9, 0.0), (10, -6.0), (20, -6.0), (21, 0.0), (99, 0.0)]);
    }
}
