"""Monta o segundo vídeo de demonstração, mais dinâmico, SÓ com chamadas ao MCP (stdio).

Este é o script que o Claude Opus 5.5 usou para produzir o vídeo
publicado no YouTube (link no README). Reaproveita o cliente (`mcp_stdio.py`) e a classe
`Demo` de `construir_demo.py` (o vídeo do Sonnet 5.5), e acrescenta movimento: entradas com
"estalo" (passa do ponto e volta), cortinas entre as cenas, palavras em sequência rápida,
barras de áudio que pulsam e uma barra de progresso do vídeo inteiro. Termina com o crédito
"criado pela crom.run".

uso:  python3 construir_demo_dinamico.py <saida.kdenlive> [caminho/do/kdenlive-mcp-plus]
      (sem o segundo argumento, usa ../../target/release/kdenlive-mcp-plus)
depois: abra o `.kdenlive` no Kdenlive, ou renderize com a ferramenta `export_render`.
"""
import os, random, sys, time
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from mcp_stdio import Servidor
from construir_demo import (Demo, FPS, S, FONTE, FUNDO, CIAN, BRANCO, CINZA, AMARELO, MAGENTA, VERDE, LARANJA,
                            ESCURO, ROXO)

# (nome, duração em quadros, tem cabeçalho numerado)
CENAS = [
    ("Abertura", 165, False), ("Em ritmo", 150, False), ("Projetos e perfis", 180, True),
    ("Biblioteca e timeline", 210, True), ("Edição de clipes", 180, True), ("Efeitos e keyframes", 180, True),
    ("Transições", 180, True), ("Velocidade e reverso", 150, True), ("Reenquadramento vertical", 180, True),
    ("Títulos e legendas", 180, True), ("Áudio e ducking", 180, True), ("Guias e desfazer", 180, True),
    ("Render e presets", 180, True), ("Análise do resultado", 150, True), ("Integração com o Kdenlive", 180, True),
    ("Encerramento", 300, False),
]
TOTAL = sum(d for _, d, _ in CENAS)
NUMERADAS = sum(1 for *_, n in CENAS if n)
VIRADA_FINAL = 145   # quadro do encerramento em que entra o "criado pela crom.run"


class Dinamico(Demo):
    # deslocamento (x, y) e escala de onde cada entrada parte
    MODOS = {"pop": (0, 0, 1.6), "esq": (-420, 0, 1), "dir": (420, 0, 1), "baixo": (0, 90, 1),
             "cima": (0, -90, 1), "voa": (1400, 0, 1), "cresce": (0, 0, 0.3)}

    def __init__(self, s):
        super().__init__(s)
        self.ordem = []      # trilhas de camada, de baixo para cima (cada nova fica por cima)
        self.usos = {}       # track_id -> [(início, fim)]
        self.fixa = None

    def camada(self, ini, fim):
        """Cada elemento fica por cima de tudo o que já está na tela no mesmo intervalo.

        A `Demo` reaproveita a primeira trilha livre, que pode estar por baixo de um elemento
        colocado antes (o texto sumia atrás da faixa em que devia aparecer)."""
        if self.fixa:
            return self.fixa
        cruza = lambda tid: any(a < fim and ini < b for a, b in self.usos[tid])
        acima = max((i for i, tid in enumerate(self.ordem) if cruza(tid)), default=-1) + 1
        for tid in self.ordem[acima:]:
            if not cruza(tid):
                break
        else:
            tid = self.c("timeline_add_track", {"kind": "video"})["track_id"]
            self.c("track_set_name", {"track_id": tid, "name": f"Camada {len(self.ordem) + 1}"})
            self.ordem.append(tid)
            self.usos[tid] = []
        self.usos[tid].append((ini, fim))
        self.fim[tid] = max(fim, self.fim.get(tid, 0))
        return tid

    def colocar(self, prod, ini, dur, kfs, tid=None):
        """Com `tid`, coloca numa trilha fixa (cortinas e barra de progresso, sempre por cima)."""
        self.fixa = tid
        try:
            return super().colocar(prod, ini, dur, kfs)
        finally:
            self.fixa = None

    @classmethod
    def movimento(cls, cx, cy, w, h, dur, modo="pop", ent=8, sai=6, op=1, rot=None):
        if dur - sai <= ent:
            ent = sai = max(1, (dur - 1) // 3)
        if modo == "estica":   # cresce a partir da borda esquerda
            kfs = [(0, cx - w / 2 + 1, cy, 2, h, 0), (ent, cx, cy, w, h, op)]
            fim = (dur - 1, cx, cy, w, h, 0)
        else:
            dx, dy, a = cls.MODOS[modo]
            kfs = [(0, cx + dx, cy + dy, w * a, h * a, 0)]
            if ent + 4 < dur - sai:   # passa um pouco do ponto e volta: o "estalo"
                b = 1 + (1 - a) * 0.08
                kfs += [(ent, cx - dx * 0.04, cy - dy * 0.04, w * b, h * b, op), (ent + 4, cx, cy, w, h, op)]
            else:
                kfs.append((ent, cx, cy, w, h, op))
            if dx or dy:   # sai continuando o movimento
                fim = (dur - 1, cx - dx * 0.5, cy - dy * 0.5, w, h, 0)
            else:
                fim = (dur - 1, cx, cy, w * 0.85, h * 0.85, 0)
        kfs += [(dur - sai, cx, cy, w, h, op), fim]
        if rot is not None:
            kfs = [k + (rot,) for k in kfs]
        return kfs

    def palavra(self, txt, tam, cor, cx, cy, ini, dur, modo="pop", ent=8, sai=6, e=1.0, kfs=None):
        dur = int(dur)
        largura = 0.602 * tam * len(txt)   # largura de uma fonte monoespaçada
        if largura > self.LARGURA_MAX:
            tam = tam * self.LARGURA_MAX / largura
        r = self.c("title_add", {"text": txt, "duration_frames": dur, "font_family": FONTE,
                                 "font_size": max(8, int(tam * S)), "color": cor})
        self.c("bin_rename", {"clip_id": r["clip_id"], "name": txt[:40]})
        kfs = kfs or self.movimento(cx, cy, 1920 * e, 1080 * e, dur, modo, ent, sai)
        return self.colocar(r["clip_id"], ini, dur, kfs)

    def bloco(self, cor, cx, cy, w, h, ini, dur, modo="cresce", ent=8, sai=6, op=1, rot=None, kfs=None, tid=None):
        dur = int(dur)
        kfs = kfs or self.movimento(cx, cy, w, h, dur, modo, ent, sai, op, rot)
        return self.colocar(self.forma(cor), ini, dur, kfs, tid)

    def cabeca(self, t0, L, n, titulo, sub=None):
        self.palavra(f"{n:02d}/{NUMERADAS:02d}", 30, CIAN, -820, -478, t0 + 4, L - 8, "esq")
        self.palavra(titulo, 72, BRANCO, 0, -395, t0 + 2, L - 6, "esq", ent=10)
        self.bloco(CIAN, 0, -340, 560, 6, t0 + 10, L - 14, "estica", ent=10)
        if sub:
            self.palavra(sub, 40, CINZA, 0, -282, t0 + 16, L - 22, "dir", ent=10)

    def chips(self, t0, L, nomes, ini=50, y=410):
        """Os nomes das ferramentas MCP, em etiquetas que aparecem uma a uma."""
        tam, folga, maximo = 30, 22, 1720
        larguras = [0.602 * tam * len(n) + 44 for n in nomes]
        linhas, atual = [], []
        for i, w in enumerate(larguras):
            if atual and sum(larguras[j] for j in atual) + folga * len(atual) + w > maximo:
                linhas.append(atual)
                atual = []
            atual.append(i)
        linhas.append(atual)
        for r, linha in enumerate(linhas):
            yr = y - (len(linhas) - 1 - r) * 62
            x = -(sum(larguras[i] for i in linha) + folga * (len(linha) - 1)) / 2
            for i in linha:
                cx = x + larguras[i] / 2
                x += larguras[i] + folga
                a = t0 + ini + i * 5
                dur = L - ini - i * 5 - 4
                self.bloco(ESCURO, cx, yr, larguras[i], 50, a, dur, "pop")
                self.palavra(nomes[i], tam, CIAN, cx, yr, a + 2, dur - 2, "pop")


# =========================================================================================
def c_abertura(d, t0, L):
    d.palavra("93", 380, CIAN, 0, -60, t0, 76, "pop", ent=10)
    d.palavra("ferramentas MCP", 64, BRANCO, 0, 170, t0 + 14, 62, "baixo")
    d.bloco(MAGENTA, 0, -40, 1920, 230, t0 + 72, L - 76, "estica", ent=8)
    d.palavra("KDENLIVE MCP PLUS", 120, FUNDO, 0, -40, t0 + 78, L - 84, "pop")
    d.palavra("um agente de IA edita vídeo de verdade", 46, BRANCO, 0, 150, t0 + 96, L - 100, "baixo")
    d.palavra("sem interface gráfica · formato nativo .kdenlive", 34, CINZA, 0, 225, t0 + 110, L - 114, "baixo")


def c_ritmo(d, t0, L):
    palavras = [("projetos", CIAN), ("timeline", MAGENTA), ("efeitos", AMARELO), ("títulos", VERDE),
                ("áudio", LARANJA), ("render", ROXO)]
    for i, (p, cor) in enumerate(palavras):
        a = t0 + 3 + i * 24
        d.bloco(cor, 0, 0, 1920, 300, a, 24, "esq" if i % 2 == 0 else "dir", ent=5, sai=4)
        d.palavra(p, 170, FUNDO, 0, 0, a + 2, 22, "pop", ent=5, sai=3)


def c_perfis(d, t0, L, n):
    d.cabeca(t0, L, n, "Projetos e perfis", "76 perfis prontos do MLT")
    D = L - 24
    d.bloco(CIAN, 0, 40, 640, 360, t0 + 20, D, kfs=[
        (0, 0, 40, 64, 36, 0), (10, 0, 40, 680, 382, 1), (14, 0, 40, 640, 360, 1), (52, 0, 40, 640, 360, 1),
        (62, 0, 40, 190, 360, 1), (65, 0, 40, 202, 360, 1), (100, 0, 40, 202, 360, 1), (110, 0, 40, 380, 380, 1),
        (113, 0, 40, 360, 360, 1), (D - 6, 0, 40, 360, 360, 1), (D - 1, 0, 40, 216, 216, 0)])
    d.palavra("16:9 · YouTube", 44, AMARELO, 0, 260, t0 + 32, 46)
    d.palavra("9:16 · Shorts e Reels", 44, AMARELO, 0, 260, t0 + 84, 46)
    d.palavra("1:1 · Feed", 44, AMARELO, 0, 260, t0 + 134, L - 138)
    d.chips(t0, L, ["project_new", "project_set_profile", "project_list_profiles", "project_open", "project_save"])


def c_timeline(d, t0, L, n):
    d.cabeca(t0, L, n, "Biblioteca e timeline", "pastas, cores e notas · várias trilhas de vídeo e de áudio")
    pistas = [("V2", -170), ("V1", -90), ("A1", -10), ("A2", 70)]
    for i, (nome, y) in enumerate(pistas):
        d.bloco(ESCURO, 60, y, 1560, 60, t0 + 20 + i * 4, L - 26 - i * 4, "estica", ent=10)
        d.palavra(nome, 32, CINZA, -800, y, t0 + 24 + i * 4, L - 30 - i * 4, "esq")
    clipes = [(0, CIAN, -640, 260), (0, MAGENTA, -300, 420), (1, VERDE, -700, 540), (1, AMARELO, -120, 480),
              (1, LARANJA, 400, 300), (2, ROXO, -700, 900), (2, CIAN, 240, 420), (3, CINZA, -500, 800)]
    for k, (p, cor, x0, w) in enumerate(clipes):
        a = 40 + k * 7
        d.bloco(cor, x0 + w / 2, pistas[p][1], w, 46, t0 + a, L - a - 6, "voa", ent=10)
    D = L - 116
    d.bloco(BRANCO, -720, -50, 4, 340, t0 + 110, D, kfs=[
        (0, -720, -50, 4, 340, 0), (6, -720, -50, 4, 340, 1), (70, 780, -50, 4, 340, 1), (D - 1, 780, -50, 4, 340, 0)])
    d.chips(t0, L, ["bin_import_clip", "bin_create_folder", "timeline_add_track", "timeline_add_clip",
                    "track_set_locked", "track_set_muted"], ini=60)


def c_edicao(d, t0, L, n):
    d.cabeca(t0, L, n, "Edição de clipes", "dividir · mover · aparar · remover · montar em lote")
    d.bloco(ESCURO, 0, 20, 1500, 100, t0 + 14, L - 18, "estica", ent=10)
    d.bloco(CIAN, 0, 20, 900, 64, t0 + 20, 30, ent=8, sai=2)
    d.bloco(BRANCO, 0, 20, 6, 150, t0 + 44, 14, ent=4, sai=6)
    D = L - 52
    d.bloco(CIAN, -225, 20, 450, 64, t0 + 48, D, kfs=[
        (0, -225, 20, 450, 64, 1), (36, -225, 20, 450, 64, 1), (46, -150, 20, 300, 64, 1),
        (D - 8, -150, 20, 300, 64, 1), (D - 1, -150, 20, 300, 64, 0)])
    d.bloco(MAGENTA, 225, 20, 450, 64, t0 + 48, 82, kfs=[
        (0, 225, 20, 450, 64, 1), (14, 225, 20, 450, 64, 1), (26, 420, 20, 450, 64, 1), (70, 420, 20, 450, 64, 1),
        (81, 420, 20, 90, 64, 0)])
    for t, a in [("dividir", 44), ("mover", 62), ("aparar", 84), ("remover", 118)]:
        d.palavra(t, 64, AMARELO, 0, 190, t0 + a, 30, ent=6, sai=5)
    d.chips(t0, L, ["timeline_split_clip", "timeline_move_clip", "timeline_trim_clip", "timeline_remove_clip",
                    "timeline_assemble_ranges"], ini=60)


def c_efeitos(d, t0, L, n):
    d.cabeca(t0, L, n, "Efeitos e keyframes", "qualquer filtro do MLT, animado no tempo")
    D = L - 24
    d.bloco(MAGENTA, -560, 40, 200, 200, t0 + 20, D, kfs=[
        (0, -560, 40, 100, 100, 0, 0), (10, -560, 40, 200, 200, 1, 0), (45, -200, -60, 260, 260, 1, 90),
        (80, 250, 90, 180, 180, 1, 200), (115, 560, 0, 300, 300, 1, 360), (140, 0, 40, 140, 140, 1, 540),
        (D - 6, 0, 40, 140, 140, 1, 540), (D - 1, 0, 40, 60, 60, 0, 540)])
    d.bloco(ESCURO, 0, 260, 1200, 6, t0 + 20, D, "estica", ent=10)
    for f in [10, 45, 80, 115, 140]:
        d.bloco(AMARELO, -600 + f / 150 * 1200, 260, 26, 26, t0 + 20 + f, D - f, "pop", ent=5, rot=45)
    d.palavra("cada ◆ é um keyframe", 32, CINZA, 0, 320, t0 + 60, L - 64, "baixo")
    d.chips(t0, L, ["filter_add", "filter_set_param", "keyframe_set", "clip_set_transform", "clip_set_opacity"],
            ini=70)


def c_transicoes(d, t0, L, n):
    d.cabeca(t0, L, n, "Transições", "dissolver · cortina · zoom · qualquer transição do MLT")
    D, y, w, h = L - 24, 40, 420, 236

    def painel(cor, cx, kfs):
        d.bloco(cor, cx, y, w, h, t0 + 20, D, kfs=[(f, cx + dx, y, w * e, h * e, o) for f, dx, e, o in kfs])

    # dissolver: um some enquanto o outro aparece
    painel(CIAN, -560, [(0, 0, 0.6, 0), (10, 0, 1, 1), (40, 0, 1, 1), (70, 0, 1, 0), (D - 1, 0, 1, 0)])
    painel(MAGENTA, -560, [(0, 0, 1, 0), (40, 0, 1, 0), (70, 0, 1, 1), (D - 8, 0, 1, 1), (D - 1, 0, 1, 0)])
    # cortina: o novo cresce da borda esquerda por cima do antigo
    painel(VERDE, 0, [(0, 0, 0.6, 0), (10, 0, 1, 1), (D - 8, 0, 1, 1), (D - 1, 0, 1, 0)])
    d.bloco(AMARELO, 0, y, w, h, t0 + 20, D, kfs=[
        (0, -w / 2 + 1, y, 2, h, 0), (60, -w / 2 + 1, y, 2, h, 0), (61, -w / 2 + 1, y, 2, h, 1),
        (95, 0, y, w, h, 1), (D - 8, 0, y, w, h, 1), (D - 1, 0, y, w, h, 0)])
    # zoom: um encolhe, o outro cresce do centro
    painel(ROXO, 560, [(0, 0, 0.6, 0), (10, 0, 1, 1), (80, 0, 1, 1), (100, 0, 0.1, 0), (D - 1, 0, 0.1, 0)])
    painel(LARANJA, 560, [(0, 0, 0.1, 0), (90, 0, 0.1, 0), (110, 0, 1.05, 1), (114, 0, 1, 1), (D - 8, 0, 1, 1),
                          (D - 1, 0, 1, 0)])
    for i, (t, a) in enumerate([("dissolver", 40), ("cortina", 60), ("zoom", 80)]):
        d.palavra(t, 40, BRANCO, -560 + i * 560, 200, t0 + a, L - a - 4)
    d.chips(t0, L, ["transition_add", "transition_set", "transition_list", "transition_remove"], ini=90)


def c_velocidade(d, t0, L, n):
    d.cabeca(t0, L, n, "Velocidade e reverso", "câmera lenta, acelerado e de trás para frente")
    D = L - 32
    pistas = [("0,5×", -150, CIAN, 100, False), ("1×", -60, VERDE, 50, False), ("2×", 30, AMARELO, 25, False),
              ("reverso", 120, MAGENTA, 50, True)]
    for i, (nome, y, cor, T, rev) in enumerate(pistas):
        d.palavra(nome, 36, BRANCO, -720, y, t0 + 16 + i * 3, L - 20 - i * 3, "esq")
        d.bloco(ESCURO, 60, y, 1100, 10, t0 + 16 + i * 3, L - 20 - i * 3, "estica", ent=10)
        a, b = (-460, 580) if not rev else (580, -460)
        d.bloco(cor, a, y, 56, 56, t0 + 28, D, kfs=[
            (0, a, y, 30, 30, 0), (6, a, y, 56, 56, 1), (6 + T, b, y, 56, 56, 1), (D - 8, b, y, 56, 56, 1),
            (D - 1, b, y, 56, 56, 0)])
    d.palavra("o clipe vira um produtor timewarp do MLT", 32, CINZA, 0, 250, t0 + 50, L - 54, "baixo")
    d.chips(t0, L, ["clip_set_speed", "clip_reverse"], ini=60)


def c_vertical(d, t0, L, n):
    d.cabeca(t0, L, n, "Reenquadramento 16:9 → 9:16", "do YouTube para Shorts e Reels, com foco ajustável")
    d.bloco(ESCURO, -260, 60, 800, 450, t0 + 18, L - 22)
    d.bloco(CIAN, -480, 60, 120, 120, t0 + 26, L - 30, "pop")
    d.bloco(AMARELO, -60, 60, 120, 120, t0 + 30, L - 34, "pop")
    D = L - 54
    d.bloco(MAGENTA, -480, 60, 253, 450, t0 + 50, D, kfs=[
        (0, -480, 60, 253, 450, 0), (8, -480, 60, 253, 450, 0.5), (45, -480, 60, 253, 450, 0.5),
        (75, -60, 60, 253, 450, 0.5), (D - 8, -60, 60, 253, 450, 0.5), (D - 1, -60, 60, 253, 450, 0)])
    d.palavra("→", 90, CIAN, 230, 60, t0 + 50, L - 54, "esq")
    d.bloco(ESCURO, 560, 60, 253, 450, t0 + 50, L - 54)
    d.bloco(CIAN, 560, 60, 120, 120, t0 + 60, 56, "pop")
    d.bloco(AMARELO, 560, 60, 120, 120, t0 + 122, L - 126, "pop")
    d.palavra("16:9", 34, CINZA, -260, 320, t0 + 30, L - 34, "baixo")
    d.palavra("9:16", 34, CINZA, 560, 320, t0 + 56, L - 60, "baixo")
    d.chips(t0, L, ["clip_reframe_vertical", "modo fill / fit", "focus_x / focus_y"], ini=80)


def c_titulos(d, t0, L, n):
    d.cabeca(t0, L, n, "Títulos e legendas", "títulos nativos do Kdenlive · legendas SRT")
    d.palavra("Título animado", 110, AMARELO, 0, -130, t0 + 18, L - 22, "pop", ent=10)
    d.palavra("fonte · tamanho · cor · animação", 36, CINZA, 0, -40, t0 + 32, L - 36, "baixo")
    d.bloco(ESCURO, 0, 190, 1300, 90, t0 + 40, L - 44, "estica", ent=10)
    falas = [("Olá! Esta legenda veio de um arquivo SRT.", "00:00:01,000 --> 00:00:03,000"),
             ("Cada fala vira um título sincronizado.", "00:00:03,000 --> 00:00:05,000"),
             ("E tudo volta para SRT quando você quiser.", "00:00:05,000 --> 00:00:07,000")]
    for i, (fala, tempo) in enumerate(falas):
        a = t0 + 50 + i * 40
        dur = 38 if i < 2 else L - 54 - i * 40
        d.palavra(fala, 40, BRANCO, 0, 190, a, dur, "baixo", ent=6, sai=5)
        d.palavra(tempo, 28, CINZA, 0, 275, a, dur, "pop", ent=6, sai=5)
    d.chips(t0, L, ["title_add", "title_update", "subtitle_import_srt", "subtitle_export_srt"], ini=60)


def c_audio(d, t0, L, n):
    d.cabeca(t0, L, n, "Áudio: níveis e ducking", "a música abaixa sozinha quando a voz entra")
    rng = random.Random(7)
    D, ini_duck, fim_duck = L - 24, 55, 125
    d.palavra("voz", 32, CINZA, -760, -40, t0 + 18, L - 22, "esq")
    d.palavra("música", 32, CINZA, -760, 150, t0 + 18, L - 22, "esq")
    for i in range(16):
        x = -525 + i * 70
        for cor, base, voz in [(CIAN, 210, False), (AMARELO, 20, True)]:
            kfs = []
            for f in range(0, D, 6):
                no_duck = ini_duck <= f <= fim_duck
                if voz:
                    h = rng.uniform(40, 160) if no_duck else 8
                else:
                    h = rng.uniform(60, 170) * (0.3 if no_duck else 1)
                o = 0 if f == 0 else 1
                kfs.append((f, x, base - h / 2, 50, h, o))
            kfs.append((D - 1, x, base - 4, 50, 8, 0))
            d.bloco(cor, x, base, 50, 8, t0 + 20, D, kfs=kfs)
    d.palavra("-12 dB na música enquanto a voz fala", 36, AMARELO, 0, 290, t0 + 20 + ini_duck, fim_duck - ini_duck + 6)
    d.chips(t0, L, ["audio_duck", "audio_set_levels", "audio_normalize", "track_set_muted"], ini=70)


def c_guias(d, t0, L, n):
    d.cabeca(t0, L, n, "Guias, marcadores e desfazer", "capítulos na régua · 50 níveis de desfazer")
    d.bloco(ESCURO, 0, -170, 1500, 12, t0 + 16, L - 20, "estica", ent=10)
    for i, (x, nome, cor) in enumerate([(-560, "Intro", ROXO), (-180, "Corte", VERDE), (200, "Efeito", AMARELO),
                                         (580, "Fim", MAGENTA)]):
        a = t0 + 24 + i * 8
        d.bloco(cor, x, -170, 20, 90, a, L - 28 - i * 8, "cima")
        d.palavra(nome, 36, BRANCO, x, -240, a + 2, L - 30 - i * 8, "cima")
    passos = ["1. adicionar título", "2. aplicar efeito", "3. mover clipe", "4. dividir clipe"]
    for i, p in enumerate(passos):
        a = 60 + i * 8
        fim = {2: 122, 3: 115}.get(i, L - 4)
        d.palavra(p, 40, BRANCO, 0, -60 + i * 60, t0 + a, fim - a, "esq")
    d.palavra(passos[2], 40, BRANCO, 0, 60, t0 + 145, L - 149, "esq")
    d.palavra("↶ desfazer", 54, MAGENTA, 0, 230, t0 + 112, 30)
    d.palavra("↷ refazer", 54, VERDE, 0, 230, t0 + 143, L - 147)
    d.chips(t0, L, ["guide_add", "marker_add", "clip_set_color", "session_undo", "session_redo",
                    "session_history"], ini=40)


def c_render(d, t0, L, n):
    d.cabeca(t0, L, n, "Render e presets", "o vídeo final, com o mesmo motor do Kdenlive")
    D = L - 24
    d.palavra("H.264 · H.265 · WebM VP9 · ProRes · FFV1 · GIF · WAV", 44, BRANCO, 0, -180, t0 + 20, D, kfs=[
        (0, 700, -180, 1920, 1080, 0), (10, 590, -180, 1920, 1080, 1), (D - 10, -590, -180, 1920, 1080, 1),
        (D - 1, -700, -180, 1920, 1080, 0)])
    for i, v in enumerate(["0 %", "25 %", "50 %", "75 %", "100 %"]):
        a = 30 + i * 22
        d.palavra(v, 130, VERDE if i == 4 else CIAN, 0, 10, t0 + a, 22 if i < 4 else L - a - 4, ent=4, sai=3)
    d.bloco(ESCURO, 0, 150, 1000, 30, t0 + 26, L - 30, "estica", ent=8)
    D = L - 30
    d.bloco(CIAN, 0, 150, 1000, 30, t0 + 26, D, kfs=[
        (0, -498, 150, 4, 30, 0), (4, -498, 150, 4, 30, 1), (92, 0, 150, 1000, 30, 1), (D - 8, 0, 150, 1000, 30, 1),
        (D - 1, 0, 150, 1000, 30, 0)])
    d.palavra("✓ pronto", 50, VERDE, 0, 230, t0 + 122, L - 126)
    d.chips(t0, L, ["export_render", "render_queue_add", "render_queue_start", "render_progress",
                    "export_thumbnail"], ini=40)


def c_analise(d, t0, L, n):
    d.cabeca(t0, L, n, "Análise do resultado", "conferir o que foi montado sem renderizar tudo")
    pos = [(-480 + col * 320, -80 + lin * 200) for lin in range(2) for col in range(4)]
    for k, (cx, cy) in enumerate(pos):
        a = 18 + k * 4
        d.bloco(ESCURO, cx, cy, 300, 169, t0 + a, L - a - 4, "pop")
        d.palavra(f"quadro {k * 30}", 30, CIAN, cx, cy, t0 + a + 4, L - a - 8, "pop")
    D = L - 64
    kfs = [(0, *pos[0], 330, 199, 0), (4, *pos[0], 330, 199, 0.45)]
    for k in range(1, 8):
        kfs += [(4 + k * 10 - 3, *pos[k - 1], 330, 199, 0.45), (4 + k * 10, *pos[k], 330, 199, 0.45)]
    kfs += [(D - 6, *pos[7], 330, 199, 0.45), (D - 1, *pos[7], 330, 199, 0)]
    d.bloco(MAGENTA, *pos[0], 330, 199, t0 + 60, D, kfs=kfs)
    d.palavra("folha de contato · quadro a quadro · GIF de um trecho", 34, CINZA, 0, 290, t0 + 50, L - 54, "baixo")
    d.chips(t0, L, ["preview_frame", "preview_frames", "preview_gif", "project_validate"], ini=60)


def c_integracao(d, t0, L, n):
    d.cabeca(t0, L, n, "Com o Kdenlive aberto", "você vê o vídeo sendo montado, ao vivo")
    caixas = [(-660, -60, 380, "Agente de IA", CIAN), (-160, -60, 520, "MCP · 93 ferramentas", MAGENTA),
              (400, -60, 440, "projeto .kdenlive", AMARELO), (400, 130, 440, "Kdenlive aberto", VERDE)]
    for i, (cx, cy, w, nome, cor) in enumerate(caixas):
        a = 18 + i * 10
        d.bloco(cor, cx, cy, w, 90, t0 + a, L - a - 4, "esq")
        d.palavra(nome, 32, FUNDO, cx, cy, t0 + a + 2, L - a - 6, "esq")
    d.palavra("→", 70, BRANCO, -415, -60, t0 + 40, L - 44)
    d.palavra("→", 70, BRANCO, 135, -60, t0 + 50, L - 54)
    d.palavra("↓", 70, BRANCO, 400, 35, t0 + 60, L - 64)
    trajeto = [(0, -470, -60), (12, 180, -60), (20, 400, -10), (28, 400, 85)]
    kfs = []
    for c in range(3):
        for j, (f, x, y) in enumerate(trajeto):
            kfs.append((c * 32 + f + (1 if j == 0 else 0), x, y, 22, 22, 0 if j == 0 else 1))
        kfs.append((c * 32 + 29, 400, 85, 22, 22, 0))
    d.bloco(BRANCO, -470, -60, 22, 22, t0 + 70, 100, kfs=kfs)
    d.palavra("grava a cada edição · o Kdenlive recarrega sozinho", 38, AMARELO, 0, 260, t0 + 90, L - 94, "baixo")
    d.chips(t0, L, ["project_save", "project_open", "project_import_kdenlive"], ini=100)


def c_final(d, t0, L):
    v = VIRADA_FINAL
    d.palavra("93 ferramentas  ·  103 testes automáticos", 56, CIAN, 0, -200, t0 + 6, v - 6)
    d.palavra("vídeo produzido pelo Claude Opus 5.5", 64, MAGENTA, 0, -70, t0 + 24, v - 24, "esq")
    d.palavra("usando apenas as ferramentas deste MCP", 44, BRANCO, 0, 20, t0 + 38, v - 38, "dir")
    d.palavra("github.com/crom-org/kdenlive-mcp-plus", 44, AMARELO, 0, 140, t0 + 54, v - 54, "baixo")
    d.palavra("criado pela", 54, CINZA, 0, -150, t0 + v + 12, L - v - 12, "baixo", sai=14)
    d.palavra("crom.run", 200, CIAN, 0, 0, t0 + v + 20, L - v - 20, "pop", ent=10, sai=14)
    d.bloco(MAGENTA, 0, 130, 700, 8, t0 + v + 32, L - v - 32, "estica", ent=12, sai=14)


FUNCOES = [c_abertura, c_ritmo, c_perfis, c_timeline, c_edicao, c_efeitos, c_transicoes, c_velocidade, c_vertical,
           c_titulos, c_audio, c_guias, c_render, c_analise, c_integracao, c_final]


def construir(s):
    d = Dinamico(s)
    fundo = d.c("title_add", {"text": "████", "duration_frames": TOTAL, "font_family": FONTE,
                              "font_size": int(1300 * S), "color": FUNDO})["clip_id"]
    d.c("bin_rename", {"clip_id": fundo, "name": "Fundo azul-marinho"})
    d.c("timeline_add_clip", {"track_id": "track0", "producer_id": fundo, "in_frame": 0, "out_frame": TOTAL - 1,
                              "position": 0})
    d.c("track_set_name", {"track_id": "track0", "name": "Fundo"})
    # dois quadrados enormes girando devagar atrás de tudo, o vídeo inteiro
    for j, (x0, x1, y0, y1, g) in enumerate([(-700, 600, -250, 300, 200), (650, -500, 300, -200, -160)]):
        tid = d.c("timeline_add_track", {"kind": "video"})["track_id"]
        d.c("track_set_name", {"track_id": tid, "name": f"Fundo animado {j + 1}"})
        kfs = [(f, x0 + (x1 - x0) * f / TOTAL, y0 + (y1 - y0) * f / TOTAL, 760, 760, 0.35, g * f / TOTAL)
               for f in range(0, TOTAL, 600)] + [(TOTAL - 1, x1, y1, 760, 760, 0.35, g)]
        d.bloco(ESCURO, x0, y0, 760, 760, 0, TOTAL, kfs=kfs, tid=tid)
    t0, n, inicios = 0, 0, []
    for (nome, L, numerada), f in zip(CENAS, FUNCOES):
        d.c("guide_add", {"position": t0, "comment": nome})
        if numerada:
            n += 1
            f(d, t0, L, n)
        else:
            f(d, t0, L)
        inicios.append(t0)
        print(f"  cena '{nome}': inicio {t0}, {L} quadros — {d.chamadas} chamadas, {len(d.fim)} camadas", flush=True)
        t0 += L
    # por cima de tudo: as cortinas entre as cenas e a barra de progresso do vídeo
    cortinas = [d.c("timeline_add_track", {"kind": "video"})["track_id"] for _ in range(2)]
    for tid, nome in zip(cortinas, ["Cortina 1", "Cortina 2"]):
        d.c("track_set_name", {"track_id": tid, "name": nome})
    for t in inicios[1:] + [inicios[-1] + VIRADA_FINAL]:
        for tid, cor, atraso in [(cortinas[0], CIAN, 0), (cortinas[1], MAGENTA, 3)]:
            d.bloco(cor, 0, 0, 1920, 1080, t - 9 + atraso, 18, tid=tid, kfs=[
                (0, -1920, 0, 1920, 1080, 1), (9, 0, 0, 1920, 1080, 1), (17, 1920, 0, 1920, 1080, 1)])
    tid = d.c("timeline_add_track", {"kind": "video"})["track_id"]
    d.c("track_set_name", {"track_id": tid, "name": "Progresso"})
    d.bloco(CIAN, 0, 534, 1920, 8, 0, TOTAL, tid=tid, kfs=[(0, -959, 534, 2, 8, 0.8), (TOTAL - 1, 0, 534, 1920, 8, 0.8)])
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
    print("duração (quadros):", info["duracao_quadros"], "->", info["duracao_quadros"] / FPS, "s; trilhas:",
          len(info["trilhas"]))
    s.fechar()
