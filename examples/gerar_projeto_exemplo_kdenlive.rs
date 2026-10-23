//! Gera um projeto representativo (bin com pasta e clipe, título, 3 trilhas,
//! filtro, transição, guia) usando só a camada de modelo, e grava em
//! `/tmp/kdenlive_mcp_teste_manual/projeto.kdenlive` — para abrir no Kdenlive
//! de verdade e conferir visualmente o que o motor gera. Usado para o teste
//! manual registrado em D-038 (docs/decisoes.md): hoje o Kdenlive real remove
//! os clipes da timeline e os efeitos/transições brutos ao abrir este projeto
//! (ele continua válido para o libmlt/render, só não é 100% round-trip no
//! Kdenlive ainda).
//!
//! Rodar com: cargo run --example gerar_projeto_exemplo_kdenlive
//! (o clipe de vídeo de `/tmp/kdenlive_mcp_teste_manual/clipe_teste.mp4`
//! precisa existir antes; qualquer .mp4 pequeno serve)

use kdenlive_mcp_plus::model::document::{self, TrackKind};

fn main() {
    let mut doc = document::new_document("atsc_1080p_25").expect("novo documento");

    // Bin: um clipe de vídeo real + uma pasta.
    document::add_bin_clip(
        &mut doc,
        "producer0",
        "/tmp/kdenlive_mcp_teste_manual/clipe_teste.mp4",
        "Clipe de teste",
        0,
        99,
    )
    .expect("add_bin_clip");
    let pasta = document::create_bin_folder(&mut doc, None, "Minha pasta").expect("create_bin_folder");
    document::bin_move_clip(&mut doc, "producer0", Some(&pasta)).expect("bin_move_clip");
    document::bin_clip_set_color(&mut doc, "producer0", "#ff0000").expect("bin_clip_set_color");
    document::bin_clip_set_note(&mut doc, "producer0", "Nota de teste do kdenlive-mcp-plus").expect("bin_clip_set_note");

    // Título.
    // Mesma resolução de fonte que a ferramenta title_add faz (mcp::fonts):
    // o Kdenlive só aceita sem aviso o nome exato de uma família instalada.
    let fonte = std::process::Command::new("fc-match")
        .args(["-f", "%{family[0]}", "Sans"])
        .output()
        .ok()
        .map(|s| String::from_utf8_lossy(&s.stdout).trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "Sans".to_string());
    document::add_title_clip(&mut doc, "titulo0", "kdenlive-mcp-plus", 75, &fonte, 96, "0xffffffff", document::TitleStyle::default())
        .expect("add_title_clip");

    // Trilhas: vídeo principal, vídeo de título, áudio.
    document::add_track(&mut doc, "V1", TrackKind::Video).expect("add_track V1");
    document::add_track(&mut doc, "V2", TrackKind::Video).expect("add_track V2");
    document::add_track(&mut doc, "A1", TrackKind::Audio).expect("add_track A1");

    // Clipe de vídeo na V1, com um filtro de opacidade.
    let idx_v1 = document::add_clip_to_track(&mut doc, "V1", "producer0", 0, 99, 0).expect("add_clip_to_track V1");
    document::add_filter(&mut doc, "V1", idx_v1, "brightness").expect("add_filter brightness");

    // Título na V2, sobrepondo o início da V1.
    document::add_clip_to_track(&mut doc, "V2", "titulo0", 0, 74, 0).expect("add_clip_to_track V2");

    // Transição entre V1 e V2.
    document::add_transition(&mut doc, "luma", "V1", "V2", &document::TransitionOptions::default()).expect("add_transition");

    // Guia, marcador geral e marcador de clipe (quadro 30 da mídia do clipe).
    document::add_guide(&mut doc, 25, "Guia de teste").expect("add_guide");
    document::add_marker(&mut doc, 50, "Marcador geral", None).expect("add_marker (timeline)");
    document::add_marker(&mut doc, 30, "Marcador do clipe", Some("V1:0")).expect("add_marker (clipe)");

    let xml = document::to_string_pretty(&doc).expect("to_string_pretty");
    std::fs::write("/tmp/kdenlive_mcp_teste_manual/projeto.kdenlive", &xml).expect("escrever projeto");
    println!("Projeto escrito em /tmp/kdenlive_mcp_teste_manual/projeto.kdenlive ({} bytes)", xml.len());
}
