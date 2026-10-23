"""Monta o vídeo de demonstração do kdenlive-mcp-plus SÓ com chamadas ao MCP (stdio).

Este é o script que o Claude Sonnet 5.5 usou para produzir o vídeo
`docs/demo/demonstracao-capacidades.mp4`: cerca de 2.000 chamadas às ferramentas do
servidor (títulos, trilhas, clipes, transformações com keyframes, guias), sem editar
nenhum arquivo à mão e sem mídia externa.

uso:  python3 construir_demo.py <saida.kdenlive> [caminho/do/kdenlive-mcp-plus]
      (sem o segundo argumento, usa ../../target/release/kdenlive-mcp-plus)
depois: abra o `.kdenlive` no Kdenlive, ou renderize com a ferramenta `export_render`
        (um render completo leva ~12 min; ajuste KDENLIVE_MCP_RENDER_TIMEOUT_SECS se precisar).

Sem mídia externa: textos (title_add) e retângulos (o mesmo título "████" escalado
pelo filtro qtblend) animados por keyframes. Coordenadas de referência 1920x1080,
com o centro do quadro em (0, 0).
"""
import os, sys, time
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from mcp_stdio import Servidor

W, H, FPS = 1920, 1080, 30
S = W / 1920
FONTE = "DejaVu Sans Mono"
FUNDO, CIAN, BRANCO, CINZA = "#0A1230", "#00E5FF", "#FFFFFF", "#8FA3C7"
AMARELO, MAGENTA, VERDE, LARANJA, ESCURO, ROXO = "#FFD54A", "#FF4FD8", "#3DDC84", "#FF8A3D", "#26397A", "#8E75FF"

# (nome, duração em quadros) — o início de cada cena é a soma das anteriores
CENAS = [
    ("Visão geral", 420), ("Projetos e perfis", 360), ("Biblioteca e timeline", 450),
    ("Edição de clipes", 420), ("Efeitos e keyframes", 420), ("Transições", 360),
    ("Velocidade e reverso", 420), ("Reenquadramento vertical", 420), ("Títulos e legendas", 420),
    ("Áudio e ducking", 450), ("Guias e marcadores", 330), ("Desfazer e refazer", 360),
    ("Render e presets", 450), ("Análise do resultado", 420), ("Integração com o Kdenlive", 450),
    ("Encerramento", 360),
]
TOTAL = sum(d for _, d in CENAS)


def r2(v):
    return round(v + 0.0, 2)


class Demo:
    def __init__(self, s):
        self.s = s
        self.fim = {}          # track_id -> último quadro ocupado
        self.formas = {}       # cor -> producer da forma
        self.chamadas = 0

    def c(self, ferramenta, args):
        self.chamadas += 1
        r = self.s.chamar(ferramenta, args)
        assert r.get("ok"), (ferramenta, args, r)
        return r

    def forma(self, cor):
        if cor not in self.formas:
            r = self.c("title_add", {"text": "████", "duration_frames": TOTAL + 60, "font_family": FONTE,
                                     "font_size": int(1300 * S), "color": cor})
            self.c("bin_rename", {"clip_id": r["clip_id"], "name": f"Forma {cor}"})
            self.formas[cor] = r["clip_id"]
        return self.formas[cor]

    def camada(self, ini, fim):
        for tid, ate in self.fim.items():
            if ate <= ini:
                self.fim[tid] = fim
                return tid
        tid = self.c("timeline_add_track", {"kind": "video"})["track_id"]
        self.c("track_set_name", {"track_id": tid, "name": f"Camada {len(self.fim) + 1}"})
        self.fim[tid] = fim
        return tid

    @staticmethod
    def rect(k):
        # k = (quadro, cx, cy, w, h, opacidade[, rotação]) em pixels de referência
        _, cx, cy, w, h, o = k[:6]
        return f"{r2(W / 2 + cx * S - w * S / 2)} {r2(H / 2 + cy * S - h * S / 2)} {r2(w * S)} {r2(h * S)} {o}"

    def colocar(self, prod, ini, dur, kfs):
        tid = self.camada(ini, ini + dur)
        r = self.c("timeline_add_clip", {"track_id": tid, "producer_id": prod, "in_frame": 0,
                                         "out_frame": dur - 1, "position": ini})
        ref = r["clip_ref"]
        k0 = kfs[0]
        x, y, w, h, _ = [float(v) for v in self.rect(k0).split()]
        f = self.c("clip_set_transform", {"clip_ref": ref, "x": x, "y": y, "width": w, "height": h})["filter_id"]
        # sem `distort` o qtblend mantém a proporção 16:9 do quadro dentro do retângulo
        self.c("filter_set_param", {"clip_ref": ref, "filter_id": f, "param": "distort", "value": "1"})
        self.c("filter_set_param", {"clip_ref": ref, "filter_id": f, "param": "rotate_center", "value": "1"})
        for k in kfs:
            self.c("keyframe_set", {"clip_ref": ref, "filter_id": f, "param": "rect", "frame": int(k[0]),
                                    "value": self.rect(k)})
        for k in kfs:
            if len(k) > 6:
                self.c("keyframe_set", {"clip_ref": ref, "filter_id": f, "param": "rotation", "frame": int(k[0]),
                                        "value": str(k[6])})
        return ref

    # --- elementos -------------------------------------------------------------------------
    LARGURA_MAX = 1780  # px de referência: o texto nunca encosta nas bordas

    def texto(self, txt, tam, cor, cx, cy, ini, dur, ent=14, sai=12, desliza=26):
        dur = int(dur)
        if tam <= 46:                      # textos pequenos eram difíceis de ler: +20 %
            tam = tam * 1.2
        largura = 0.602 * tam * len(txt)   # largura de uma fonte monoespaçada
        if self.LARGURA_MAX < largura <= self.LARGURA_MAX / 0.8:   # passou pouco: só encolhe um pouco
            tam = tam * self.LARGURA_MAX / largura
            largura = self.LARGURA_MAX
        if largura > self.LARGURA_MAX and " · " in txt:
            partes = txt.split(" · ")
            meio = (len(partes) + 1) // 2
            for i, linha in enumerate((" · ".join(partes[:meio]), " · ".join(partes[meio:]))):
                self.texto(linha, tam, cor, cx, cy + (i - 0.5) * tam * 1.3, ini, dur, ent, sai, desliza)
            return
        if largura > self.LARGURA_MAX:
            tam = tam * self.LARGURA_MAX / largura
        r = self.c("title_add", {"text": txt, "duration_frames": dur, "font_family": FONTE,
                                 "font_size": max(8, int(tam * S)), "color": cor})
        self.c("bin_rename", {"clip_id": r["clip_id"], "name": txt[:40]})
        kfs = [(0, cx, cy + desliza, 1920, 1080, 0), (ent, cx, cy, 1920, 1080, 1),
               (dur - sai, cx, cy, 1920, 1080, 1), (dur - 1, cx, cy, 1920, 1080, 0)]
        return self.colocar(r["clip_id"], ini, dur, kfs)

    def caixa(self, cor, cx, cy, w, h, ini, dur, kfs=None, ent=12, sai=12, pop=True):
        dur = int(dur)
        if kfs is None:
            a = 0.6 if pop else 1.0
            kfs = [(0, cx, cy, w * a, h * a, 0), (ent, cx, cy, w, h, 1),
                   (dur - sai, cx, cy, w, h, 1), (dur - 1, cx, cy, w, h, 0)]
        return self.colocar(self.forma(cor), ini, dur, kfs)

    def cabecalho(self, t0, L, titulo, sub=None, tam=76):
        self.texto(titulo, tam, CIAN, 0, -395, t0, L - 6)
        self.caixa(CIAN, 0, -330, 520, 5, t0 + 8, L - 14, pop=False)
        if sub:
            self.texto(sub, 44, CINZA, 0, -262, t0 + 20, L - 26)


# =========================================================================================
def cena_visao_geral(d, t0, L):
    d.cabecalho(t0, L, "KDENLIVE  MCP  PLUS", "um agente de IA edita vídeo de verdade", 96)
    itens = [("93 ferramentas MCP", CIAN), ("salva no formato nativo do Kdenlive (.kdenlive)", BRANCO),
             ("renderiza com o mesmo motor do Kdenlive (libmlt)", BRANCO), ("funciona sem interface gráfica", BRANCO)]
    for i, (t, cor) in enumerate(itens):
        d.texto(t, 50, cor, 0, -70 + i * 90, t0 + 70 + i * 50, L - 76 - i * 50)


def cena_perfis(d, t0, L):
    d.cabecalho(t0, L, "Projetos e perfis de vídeo", "76 perfis prontos: SD · HD · Full HD · 4K · vertical · quadrado")
    formas = [(-560, CIAN, 480, 270, "Full HD 16:9 · YouTube"), (-10, MAGENTA, 152, 270, "Vertical 9:16 · Shorts"),
              (470, AMARELO, 270, 270, "Quadrado 1:1 · Feed")]
    for i, (cx, cor, w, h, rot) in enumerate(formas):
        ini = t0 + 60 + i * 45
        d.caixa(cor, cx, 60, w, h, ini, L - 66 - i * 45)
        d.texto(rot, 30, CINZA, cx, 245, ini + 20, L - 86 - i * 45)
    d.texto("project_new · project_set_profile · project_list_profiles", 34, CINZA, 0, 380, t0 + 200, L - 206)


def cena_timeline(d, t0, L):
    d.cabecalho(t0, L, "Biblioteca e timeline", "importar mídia, organizar, trilhas de vídeo e de áudio")
    linhas = [("V2", -110), ("V1", -30), ("A1", 50), ("A2", 130)]
    for nome, y in linhas:
        d.caixa(ESCURO, 40, y, 1500, 56, t0 + 40, L - 46, pop=False)
        d.texto(nome, 30, CINZA, -810, y, t0 + 44, L - 50, desliza=0)
    clipes = [(-110, CIAN, -560, 230, 80), (-110, MAGENTA, -80, 340, 120), (-30, VERDE, -700, 580, 160),
              (-30, AMARELO, -100, 520, 190), (-30, LARANJA, 440, 260, 220), (50, ROXO, -700, 1000, 250),
              (130, CINZA, -300, 700, 280)]
    for y, cor, x0, w, ini in clipes:
        cx = x0 + w / 2
        dur = L - (ini) - 10
        d.caixa(cor, cx, y, w, 44, t0 + ini, dur, kfs=[(0, cx - 120, y, w, 44, 0), (16, cx, y, w, 44, 1),
                                                         (dur - 12, cx, y, w, 44, 1), (dur - 1, cx, y, w, 44, 0)])
    dur = L - 300
    d.caixa(BRANCO, -740, 10, 4, 340, t0 + 290, dur, kfs=[(0, -740, 10, 4, 340, 1), (dur - 14, 740, 10, 4, 340, 1),
                                                           (dur - 1, 740, 10, 4, 340, 0)])
    d.texto("biblioteca com pastas, cores e notas · várias trilhas · bloquear, ocultar, silenciar", 32, CINZA, 0, 270, t0 + 110, L - 116)
    d.texto("bin_import_clip · timeline_add_clip · timeline_add_track", 34, CINZA, 0, 345, t0 + 150, L - 156)


def cena_edicao(d, t0, L):
    d.cabecalho(t0, L, "Edição de clipes", "dividir · aparar · mover · remover · montar em lote")
    d.caixa(ESCURO, 0, 30, 1500, 90, t0 + 40, L - 46, pop=False)
    d.caixa(CIAN, 0, 30, 900, 60, t0 + 60, 80)                                  # clipe inteiro
    d.caixa(BRANCO, 0, 30, 5, 120, t0 + 130, 30, pop=False)                     # linha de corte
    d.caixa(CIAN, -230, 30, 440, 60, t0 + 140, 190, kfs=[(0, -230, 30, 440, 60, 1), (60, -230, 30, 440, 60, 1),
                                                         (110, -280, 30, 340, 60, 1), (189, -280, 30, 340, 60, 0)])
    d.caixa(MAGENTA, 230, 30, 440, 60, t0 + 140, 190, kfs=[(0, 230, 30, 440, 60, 1), (50, 330, 30, 440, 60, 1),
                                                           (100, 400, 30, 300, 60, 1), (160, 400, 30, 300, 60, 1),
                                                           (189, 400, 30, 300, 0.1, 0)])
    legendas = [("dividir", 120), ("mover", 175), ("aparar", 220), ("remover", 265)]
    for i, (t, ini) in enumerate(legendas):
        d.texto(t, 44, AMARELO, -450 + i * 300, 190, t0 + ini, 55)
    d.texto("timeline_split_clip · timeline_move_clip · timeline_trim_clip · timeline_assemble_ranges", 32, CINZA, 0, 330, t0 + 120, L - 126)


def cena_efeitos(d, t0, L):
    d.cabecalho(t0, L, "Efeitos e keyframes", "posição · escala · rotação · opacidade, animados no tempo")
    n = L - 66
    d.caixa(MAGENTA, -500, 20, 200, 200, t0 + 60, n,
            kfs=[(0, -500, 20, 120, 120, 0, 0), (20, -500, 20, 200, 200, 1, 0), (100, 0, 20, 320, 320, 1, 120),
                 (190, 500, 20, 200, 200, 1, 360), (260, 0, 20, 110, 110, 1, 540), (n - 1, 0, 20, 110, 110, 0, 540)])
    d.caixa(ESCURO, 0, 250, 1200, 6, t0 + 60, n, pop=False)
    for i, qd in enumerate([20, 100, 190, 260]):
        cx = -600 + (qd / 270) * 1200
        d.caixa(AMARELO, cx, 250, 24, 24, t0 + 60 + qd, n - qd,
                kfs=[(0, cx, 250, 10, 10, 0, 45), (10, cx, 250, 24, 24, 1, 45), (n - qd - 14, cx, 250, 24, 24, 1, 45),
                     (n - qd - 1, cx, 250, 24, 24, 0, 45)])
    d.texto("cada diamante é um keyframe", 32, CINZA, 0, 310, t0 + 200, L - 206)
    d.texto("filter_add · keyframe_set · clip_set_transform · clip_set_opacity", 32, CINZA, 0, 380, t0 + 230, L - 236)


def cena_transicoes(d, t0, L):
    d.cabecalho(t0, L, "Transições", "dissolver, cortinas e qualquer transição do MLT")
    n = L - 66
    d.caixa(CIAN, -330, 40, 380, 230, t0 + 60, n, kfs=[(0, -330, 40, 380, 230, 0), (12, -330, 40, 380, 230, 1),
                                                       (70, -330, 40, 380, 230, 1), (130, -330, 40, 380, 230, 0.0),
                                                       (n - 1, -330, 40, 380, 230, 0)])
    d.caixa(MAGENTA, -330, 40, 380, 230, t0 + 60, n, kfs=[(0, -330, 40, 380, 230, 0), (70, -330, 40, 380, 230, 0),
                                                          (130, -330, 40, 380, 230, 1), (n - 14, -330, 40, 380, 230, 1),
                                                          (n - 1, -330, 40, 380, 230, 0)])
    d.caixa(VERDE, 330, 40, 380, 230, t0 + 60, n, kfs=[(0, 330, 40, 380, 230, 0), (12, 330, 40, 380, 230, 1),
                                                       (n - 14, 330, 40, 380, 230, 1), (n - 1, 330, 40, 380, 230, 0)])
    d.caixa(AMARELO, 330, 40, 380, 230, t0 + 60, n, kfs=[(0, 330 + 380, 40, 380, 230, 1), (70, 330 + 380, 40, 380, 230, 1),
                                                         (130, 330, 40, 380, 230, 1), (n - 14, 330, 40, 380, 230, 1),
                                                         (n - 1, 330, 40, 380, 230, 0)])
    d.texto("dissolver", 40, BRANCO, -330, 215, t0 + 90, L - 96)
    d.texto("cortina (wipe)", 40, BRANCO, 330, 215, t0 + 90, L - 96)
    d.texto("transition_add · transition_set · luma, composite, qtblend…", 34, CINZA, 0, 345, t0 + 150, L - 156)


def cena_velocidade(d, t0, L):
    d.cabecalho(t0, L, "Velocidade e reverso", "câmera lenta, acelerado e tocar de trás para frente")
    linhas = [("0,5×", -110, CIAN, 300, False), ("1×", -20, VERDE, 150, False), ("2×", 70, AMARELO, 75, False),
              ("reverso", 160, MAGENTA, 150, True)]
    for nome, y, cor, passo, rev in linhas:
        d.texto(nome, 36, BRANCO, -700, y, t0 + 50, L - 56, desliza=0)
        d.caixa(ESCURO, 40, y, 1100, 10, t0 + 50, L - 56, pop=False)
        a, b = (-480, 560) if not rev else (560, -480)
        d.caixa(cor, a, y, 56, 56, t0 + 70, passo + 24,
                kfs=[(0, a, y, 30, 30, 0), (10, a, y, 56, 56, 1), (passo, b, y, 56, 56, 1), (passo + 23, b, y, 56, 56, 0)])
    d.texto("clip_set_speed · clip_reverse", 36, CINZA, 0, 300, t0 + 120, L - 126)
    d.texto("o clipe é envolvido num produtor timewarp do MLT", 32, CINZA, 0, 360, t0 + 150, L - 156)


def cena_reenquadro(d, t0, L):
    d.cabecalho(t0, L, "Reenquadramento 16:9 → 9:16", "da tela larga para o vertical, com foco ajustável")
    n = L - 66
    d.caixa(ESCURO, -380, 50, 640, 360, t0 + 60, n)
    d.caixa(CIAN, -560, 50, 110, 110, t0 + 70, n - 10, kfs=[(0, -560, 50, 110, 110, 0), (14, -560, 50, 110, 110, 1),
                                                          (n - 24, -560, 50, 110, 110, 1), (n - 11, -560, 50, 110, 110, 0)])
    d.caixa(AMARELO, -200, 50, 110, 110, t0 + 70, n - 10, kfs=[(0, -200, 50, 110, 110, 0), (14, -200, 50, 110, 110, 1),
                                                             (n - 24, -200, 50, 110, 110, 1), (n - 11, -200, 50, 110, 110, 0)])
    d.caixa(MAGENTA, -560, 50, 202, 360, t0 + 100, n - 40,
            kfs=[(0, -560, 50, 202, 360, 0.0), (20, -560, 50, 202, 360, 0.55), (100, -560, 50, 202, 360, 0.55),
                 (170, -200, 50, 202, 360, 0.55), (230, -200, 50, 202, 360, 0.55), (n - 41, -200, 50, 202, 360, 0)])
    d.caixa(ESCURO, 330, 50, 202, 360, t0 + 60, n)
    d.caixa(CIAN, 330, 50, 110, 110, t0 + 100, n - 40, kfs=[(0, 330, 50, 110, 110, 0), (20, 330, 50, 110, 110, 1),
                                                          (100, 330, 50, 110, 110, 1), (130, 330, 50, 110, 110, 0),
                                                          (n - 41, 330, 50, 110, 110, 0)])
    d.caixa(AMARELO, 330, 50, 110, 110, t0 + 100, n - 40, kfs=[(0, 330, 50, 110, 110, 0), (110, 330, 50, 110, 110, 0),
                                                             (170, 330, 50, 110, 110, 1), (230, 330, 50, 110, 110, 1),
                                                             (n - 41, 330, 50, 110, 110, 0)])
    d.texto("→", 90, CIAN, -30, 50, t0 + 90, L - 96, desliza=0)
    d.texto("16:9", 34, CINZA, -380, 270, t0 + 80, L - 86)
    d.texto("9:16", 34, CINZA, 330, 270, t0 + 80, L - 86)
    d.texto("clip_reframe_vertical · modo fill/fit · focus_x / focus_y", 34, CINZA, 0, 365, t0 + 150, L - 156)


def cena_titulos(d, t0, L):
    d.cabecalho(t0, L, "Títulos e legendas", "títulos nativos do Kdenlive e legendas SRT")
    d.texto("Título animado", 70, AMARELO, -400, -90, t0 + 70, 250, ent=24, desliza=60)
    d.texto("fonte, tamanho, cor e animação", 32, CINZA, -400, -20, t0 + 100, 220)
    d.caixa(ESCURO, 0, 230, 1100, 80, t0 + 60, L - 66, pop=False)
    falas = ["Olá! Esta legenda veio de um arquivo SRT.", "Cada fala vira um título sincronizado.",
             "E dá para exportar tudo de volta em SRT."]
    for i, t in enumerate(falas):
        d.texto(t, 38, BRANCO, 0, 230, t0 + 90 + i * 95, 85, ent=8, sai=8, desliza=8)
    d.texto("1\n00:00:01,000 --> 00:00:03,000".split("\n")[1], 30, CINZA, 390, -40, t0 + 90, L - 96)
    d.texto("title_add · title_update · subtitle_import_srt · subtitle_export_srt", 32, CINZA, 0, 365, t0 + 150, L - 156)


def cena_audio(d, t0, L):
    d.cabecalho(t0, L, "Áudio: níveis, normalização e ducking", "o volume da música cai sozinho quando a voz fala", 64)
    alta = "▁▃▅▇█▇▅▃▁▂▄▆█▆▄▂▁▃▅▇█▇▅▃▁▂▄▆█▆▄▂▁"
    baixa = "▁▁▂▂▃▂▂▁▁▁▂▂▂▃▂▂▁▁▁▂▂▃▂▂▁▁▁▂▂▂▃▂▂▁"
    voz = "▂▅▇▃▁▄█▆▂▁▃▇▅▂▆█▃▁▄▇▅▂▁▃▆█▄▂▅▇▃▁▄▆▂"
    n = L - 66
    d.texto("música", 32, CINZA, -820, 120, t0 + 60, n, desliza=0)
    d.texto("voz", 32, CINZA, -820, -10, t0 + 60, n, desliza=0)
    d.texto(alta, 64, CIAN, 40, 120, t0 + 60, 150, ent=10, sai=20, desliza=0)
    d.texto(baixa, 64, CIAN, 40, 120, t0 + 180, 150, ent=24, sai=24, desliza=0)
    d.texto(alta, 64, CIAN, 40, 120, t0 + 300, n - 240 + 6, ent=24, desliza=0)
    d.texto(voz, 64, AMARELO, 40, -10, t0 + 170, 140, ent=14, sai=18, desliza=0)
    d.texto("-12 dB enquanto a voz fala", 34, AMARELO, 0, 250, t0 + 190, 110)
    d.texto("audio_duck · audio_set_levels · audio_normalize", 34, CINZA, 0, 345, t0 + 120, L - 126)


def cena_guias(d, t0, L):
    d.cabecalho(t0, L, "Guias e marcadores", "capítulos na régua, notas e cores por clipe")
    d.caixa(ESCURO, 40, 40, 1500, 14, t0 + 40, L - 46, pop=False)
    marcas = [(-560, "Intro", ROXO), (-150, "Corte", VERDE), (260, "Efeito", AMARELO), (640, "Fim", MAGENTA)]
    for i, (x, nome, cor) in enumerate(marcas):
        ini = t0 + 70 + i * 50
        d.caixa(cor, x, 40, 22, 130, ini, L - 76 - i * 50, pop=False)
        d.texto(nome, 42, BRANCO, x, -60, ini + 6, L - 82 - i * 50)
    d.texto("guide_add · marker_add · clip_set_color · clip_add_note", 34, CINZA, 0, 250, t0 + 130, L - 136)


def cena_historico(d, t0, L):
    d.cabecalho(t0, L, "Desfazer e refazer", "50 níveis, cada passo com sua descrição")
    passos = ["adicionar título", "aplicar efeito", "mover clipe", "dividir clipe", "ajustar volume"]
    for i, t in enumerate(passos):
        ini = 60 + i * 30
        fim = L - 100 - (len(passos) - 1 - i) * 22 - 6 + 12 - 130 + 130
        fim = min(L - 20, 235 + (len(passos) - 1 - i) * 24)
        d.texto(f"{i + 1}.  {t}", 44, BRANCO, 0, -120 + i * 70, t0 + ini, fim - ini, ent=12, sai=10, desliza=18)
    d.texto("↶ desfazer      ↷ refazer", 54, AMARELO, 0, 270, t0 + 215, L - 221)
    d.texto("session_undo · session_redo · session_history", 34, CINZA, 0, 350, t0 + 240, L - 246)


def cena_render(d, t0, L):
    d.cabecalho(t0, L, "Render e presets", "o vídeo final, com o mesmo motor do Kdenlive")
    presets = ["H.264 alta qualidade  ·  H.264 rápido", "H.265  ·  WebM VP9", "ProRes  ·  FFV1 sem perdas",
               "GIF animado  ·  WAV só áudio"]
    for i, t in enumerate(presets):
        d.texto(t, 40, BRANCO, 0, -140 + i * 62, t0 + 60 + i * 35, L - 66 - i * 35)
    n = L - 250
    d.caixa(ESCURO, 0, 160, 900, 30, t0 + 200, n, pop=False)
    kfs = [(0, -446, 160, 8, 30, 0)]
    for f in range(10, 200, 10):
        w = max(8, 900 * min(1.0, f / 190))
        kfs.append((f, -450 + w / 2, 160, w, 30, 1))
    kfs.append((n - 1, 0, 160, 900, 30, 0))
    d.caixa(CIAN, 0, 160, 900, 30, t0 + 200, n, kfs=kfs)
    d.texto("renderizando…  100 %", 36, AMARELO, 0, 225, t0 + 215, n - 20)
    d.texto("export_render · fila de render · cancelar · render de um trecho · miniaturas", 32, CINZA, 0, 335, t0 + 140, L - 146)


def cena_analise(d, t0, L):
    d.cabecalho(t0, L, "Análise do resultado", "ver o que foi montado sem renderizar o vídeo inteiro")
    k = 0
    for linha in range(2):
        for col in range(3):
            cx, cy = -430 + col * 290, -60 + linha * 170
            ini = t0 + 60 + k * 16
            d.caixa(ESCURO, cx, cy, 256, 144, ini, L - 66 - k * 16)
            d.texto(str(k * 30), 34, CIAN, cx, cy, ini + 14, L - 86 - k * 16, desliza=0)
            k += 1
    d.caixa(MAGENTA, 640, 25, 230, 130, t0 + 190, L - 196)
    d.texto("GIF", 80, BRANCO, 640, 25, t0 + 200, L - 206)
    d.texto("folha de contato · quadro a quadro · GIF do trecho", 38, CINZA, 0, 250, t0 + 150, L - 156)
    d.texto("preview_frame · preview_frames · preview_gif", 34, CINZA, 0, 330, t0 + 180, L - 186)


def cena_integracao(d, t0, L):
    d.cabecalho(t0, L, "Com o Kdenlive aberto", "você acompanha o vídeo sendo montado")
    caixas = [(-700, -30, 420, "Agente de IA", CIAN), (-130, -30, 560, "MCP · 93 ferramentas", MAGENTA),
              (470, -30, 500, "projeto .kdenlive", AMARELO), (470, 150, 500, "Kdenlive aberto", VERDE)]
    for i, (cx, cy, w, nome, cor) in enumerate(caixas):
        ini = t0 + 60 + i * 55
        d.caixa(cor, cx, cy, w, 90, ini, L - 66 - i * 55)
        d.texto(nome, 34, FUNDO, cx, cy, ini + 14, L - 80 - i * 55, desliza=0)
    d.texto("→", 80, BRANCO, -440, -30, t0 + 100, L - 106, desliza=0)
    d.texto("→", 80, BRANCO, 185, -30, t0 + 155, L - 161, desliza=0)
    d.texto("↕", 80, BRANCO, 470, 60, t0 + 215, L - 221, desliza=0)
    d.texto("grava a cada edição", 40, AMARELO, -330, 120, t0 + 250, L - 256)
    d.texto("e o Kdenlive recarrega sozinho", 40, AMARELO, -330, 175, t0 + 265, L - 271)
    d.texto("sem fork, pelo D-Bus · as edições do agente aparecem ao vivo", 34, CINZA, 0, 330, t0 + 295, L - 301)


def cena_final(d, t0, L):
    d.cabecalho(t0, L, "Tudo testado, tudo no seu Kdenlive", None, 66)
    d.texto("93 ferramentas  ·  103 testes automáticos", 56, CIAN, 0, -120, t0 + 50, L - 56)
    d.texto("vídeo produzido pelo Claude Sonnet 5.5", 52, MAGENTA, 0, -10, t0 + 110, L - 116)
    d.texto("usando apenas as ferramentas deste sistema (MCP)", 42, BRANCO, 0, 70, t0 + 130, L - 136)
    d.texto("github.com/crom-org/kdenlive-mcp-plus", 42, AMARELO, 0, 190, t0 + 190, L - 196)


FUNCOES = [cena_visao_geral, cena_perfis, cena_timeline, cena_edicao, cena_efeitos, cena_transicoes, cena_velocidade,
           cena_reenquadro, cena_titulos, cena_audio, cena_guias, cena_historico, cena_render, cena_analise,
           cena_integracao, cena_final]


def construir(s, so_cenas=None):
    d = Demo(s)
    # fundo contínuo na trilha de vídeo padrão
    fundo = d.c("title_add", {"text": "████", "duration_frames": TOTAL, "font_family": FONTE,
                              "font_size": int(1300 * S), "color": FUNDO})["clip_id"]
    d.c("bin_rename", {"clip_id": fundo, "name": "Fundo azul-marinho"})
    d.c("timeline_add_clip", {"track_id": "track0", "producer_id": fundo, "in_frame": 0, "out_frame": TOTAL - 1, "position": 0})
    d.c("track_set_name", {"track_id": "track0", "name": "Fundo"})
    t0 = 0
    for (nome, L), f in zip(CENAS, FUNCOES):
        if so_cenas is None or nome in so_cenas:
            d.c("guide_add", {"position": t0, "comment": nome})
            f(d, t0, L)
            print(f"  cena '{nome}': inicio {t0}, {L} quadros — {d.chamadas} chamadas, {len(d.fim)} camadas", flush=True)
        t0 += L
    return d


if __name__ == "__main__":
    if len(sys.argv) < 2:
        sys.exit(__doc__)
    alvo = sys.argv[1]
    if os.path.exists(alvo):
        sys.exit(f"já existe, não sobrescrevo: {alvo}")
    binario = sys.argv[2] if len(sys.argv) > 2 else os.path.join(
        os.path.dirname(os.path.abspath(__file__)), "..", "..", "target", "release", "kdenlive-mcp-plus")
    s = Servidor(binario)
    r = s.chamar("project_new", {"profile": "atsc_1080p_30"})
    assert r.get("ok"), r
    t = time.time()
    d = construir(s)
    print(f"construído em {time.time() - t:.0f}s: {d.chamadas} chamadas MCP")
    print(s.chamar("project_save", {"path": alvo}))
    print(s.chamar("project_validate", {}))
    info = s.chamar("timeline_get_info", {})
    print("duração (quadros):", info["duracao_quadros"], "->", info["duracao_quadros"] / FPS, "s; trilhas:", len(info["trilhas"]))
    s.fechar()
