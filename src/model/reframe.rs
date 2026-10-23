//! Geometria do reenquadramento de um clipe para o quadro do projeto
//! (`clip_reframe_vertical`, D-045).
//!
//! É só aritmética, sem libmlt: dado o tamanho do vídeo de origem e o do
//! quadro do projeto, devolve o retângulo (em pixels do quadro) em que o
//! filtro `qtblend` deve desenhar o vídeo. O resultado é conferido de verdade
//! por teste de render (`tests/mcp_stdio.rs`), e não só por esta aritmética.

use std::str::FromStr;

/// Como o vídeo ocupa o quadro.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Modo {
    /// Preenche o quadro inteiro e corta o que sobra (sem faixas pretas).
    Fill,
    /// Cabe inteiro dentro do quadro, com faixas vazias nas sobras.
    Fit,
}

impl FromStr for Modo {
    type Err = String;

    fn from_str(texto: &str) -> Result<Self, Self::Err> {
        match texto {
            "fill" => Ok(Self::Fill),
            "fit" => Ok(Self::Fit),
            outro => Err(format!("mode inválido: \"{outro}\". Use \"fill\" (preenche e corta) ou \"fit\" (cabe inteiro)")),
        }
    }
}

/// Retângulo em pixels do quadro do projeto. `x`/`y` podem ser negativos
/// (a parte do vídeo que fica fora do quadro, no modo `Fill`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Retangulo {
    pub x: f64,
    pub y: f64,
    pub largura: f64,
    pub altura: f64,
}

fn duas_casas(v: f64) -> f64 {
    let r = (v * 100.0).round() / 100.0;
    if r == 0.0 { 0.0 } else { r } // evita "-0"
}

/// Calcula o retângulo do vídeo `origem` (largura, altura, já com a
/// proporção de pixel aplicada) no quadro `quadro`. `foco_x`/`foco_y` (0 a 1)
/// escolhem qual parte fica visível: 0 = alinha à esquerda/topo, 0.5 =
/// centraliza, 1 = direita/base. Valores fora de 0..=1 são limitados a ele.
pub fn calcular(
    origem: (f64, f64),
    quadro: (f64, f64),
    modo: Modo,
    foco_x: f64,
    foco_y: f64,
) -> Result<Retangulo, String> {
    let (ow, oh) = origem;
    let (qw, qh) = quadro;
    if !(ow > 0.0 && oh > 0.0 && qw > 0.0 && qh > 0.0) {
        return Err(format!(
            "tamanhos inválidos: vídeo {ow}x{oh}, quadro {qw}x{qh} (todos precisam ser maiores que zero)"
        ));
    }
    let escala_largura = qw / ow;
    let escala_altura = qh / oh;
    let escala = match modo {
        Modo::Fill => escala_largura.max(escala_altura),
        Modo::Fit => escala_largura.min(escala_altura),
    };
    let largura = ow * escala;
    let altura = oh * escala;
    let foco_x = foco_x.clamp(0.0, 1.0);
    let foco_y = foco_y.clamp(0.0, 1.0);
    Ok(Retangulo {
        x: duas_casas((qw - largura) * foco_x),
        y: duas_casas((qh - altura) * foco_y),
        largura: duas_casas(largura),
        altura: duas_casas(altura),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const VERTICAL: (f64, f64) = (1080.0, 1920.0);

    #[test]
    fn fill_de_16_9_em_vertical_preenche_a_altura_e_centraliza() {
        let r = calcular((640.0, 360.0), VERTICAL, Modo::Fill, 0.5, 0.5).unwrap();
        assert_eq!(r.altura, 1920.0);
        assert_eq!(r.largura, 3413.33);
        assert_eq!(r.y, 0.0);
        // metade do que sobra para cada lado
        assert_eq!(r.x, -1166.67);
    }

    #[test]
    fn foco_escolhe_a_parte_visivel_do_corte() {
        let esquerda = calcular((640.0, 360.0), VERTICAL, Modo::Fill, 0.0, 0.5).unwrap();
        assert_eq!(esquerda.x, 0.0);
        let direita = calcular((640.0, 360.0), VERTICAL, Modo::Fill, 1.0, 0.5).unwrap();
        assert_eq!(direita.x, -(3413.33 - 1080.0));
        // foco fora de 0..1 é limitado
        let alem = calcular((640.0, 360.0), VERTICAL, Modo::Fill, 7.0, -3.0).unwrap();
        assert_eq!(alem.x, direita.x);
        assert_eq!(alem.y, 0.0);
    }

    #[test]
    fn fit_de_16_9_em_vertical_cabe_na_largura_com_faixas_em_cima_e_embaixo() {
        let r = calcular((1920.0, 1080.0), VERTICAL, Modo::Fit, 0.5, 0.5).unwrap();
        assert_eq!(r.largura, 1080.0);
        assert_eq!(r.altura, 607.5);
        assert_eq!(r.x, 0.0);
        assert_eq!(r.y, 656.25);
    }

    #[test]
    fn video_ja_vertical_no_mesmo_tamanho_nao_muda() {
        let r = calcular((1080.0, 1920.0), VERTICAL, Modo::Fill, 0.5, 0.5).unwrap();
        assert_eq!(r, Retangulo { x: 0.0, y: 0.0, largura: 1080.0, altura: 1920.0 });
    }

    #[test]
    fn video_mais_estreito_que_o_quadro_corta_em_cima_e_embaixo() {
        // 9:20 num quadro 9:16: preenche a largura, sobra altura
        let r = calcular((900.0, 2000.0), VERTICAL, Modo::Fill, 0.5, 0.5).unwrap();
        assert_eq!(r.largura, 1080.0);
        assert_eq!(r.altura, 2400.0);
        assert_eq!(r.x, 0.0);
        assert_eq!(r.y, -240.0);
    }

    #[test]
    fn tamanhos_invalidos_geram_erro() {
        assert!(calcular((0.0, 360.0), VERTICAL, Modo::Fill, 0.5, 0.5).is_err());
        assert!(calcular((640.0, 360.0), (1080.0, 0.0), Modo::Fill, 0.5, 0.5).is_err());
    }

    #[test]
    fn modo_vem_do_texto() {
        assert_eq!("fill".parse::<Modo>(), Ok(Modo::Fill));
        assert_eq!("fit".parse::<Modo>(), Ok(Modo::Fit));
        assert!("cover".parse::<Modo>().is_err());
    }
}
