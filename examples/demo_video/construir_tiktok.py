"""Vídeo vertical 9:16 para TikTok e Reels, feito SÓ com chamadas ao MCP (stdio).

Terceiro vídeo da série. O roteiro é próprio: um gancho com palavras que entram em sequência,
uma conversa em forma de chat (o pedido e as respostas do agente), as ferramentas em fila,
três números, uma timeline vertical com a cabeça de reprodução, keyframes e um final com o
crédito "criado pela crom.run". Cada corte tem um flash e a barra de progresso fica na base.

Sem áudio: o MCP não gera som. O projeto `.kdenlive` fica aberto para você colocar música.

uso:  python3 construir_tiktok.py <saida.kdenlive> [caminho/do/kdenlive-mcp-plus]
      (sem o segundo argumento, usa ../../target/release/kdenlive-mcp-plus)
depois: abra o `.kdenlive` no Kdenlive, ou renderize com a ferramenta `export_render`.

Coordenadas em pixels do quadro 1080x1920, com o centro em (0, 0): x cresce para a direita,
y cresce para baixo.
"""
import os, sys, time
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from mcp_stdio import Servidor

W, H, FPS = 1080, 1920, 30
PERFIL = "vertical_hd_30"
FONTE, FONTE_MONO = "DejaVu Sans", "DejaVu Sans Mono"
PAPEL, LIMAO, ROSA, CEU = "#0B0B0F", "#C6FF3D", "#FF3E7F", "#3DD5FF"
BRANCO, CINZA, CARVAO = "#FFFFFF", "#7A7A88", "#1C1C24"
LARGURA_MAX = 960     # px: texto nunca encosta nas bordas (a tela tem 1080)

# (nome, duração em quadros); o início de cada cena é a soma das anteriores
CENAS = [("gancho", 120), ("conversa", 300), ("ferramentas", 300), ("numeros", 240), ("timeline", 240),
         ("keyframes", 180), ("final", 210)]
INICIO, _t = {}, 0
for _nome, _L in CENAS:
    INICIO[_nome], _t = _t, _t + _L
TOTAL = _t


def cor(hexa):
    """`#RRGGBB` para o formato `0xRRGGBBAA` do MCP."""
    return "0x" + hexa[1:].upper() + "FF"


def anima(cx, cy, w, h, dur, modo="pop", ent=8, sai=6):
    """Entrada, permanência e saída de um elemento. `modo` diz de onde ele chega."""
    ent = max(1, min(ent, (dur - 2) // 4))
    sai = max(1, min(sai, (dur - 2) // 4))
    dx, dy, escala = {"pop": (0, 0, 0.3), "estala": (0, 0, 2.4), "sobe": (0, 180, 1), "desce": (0, -180, 1),
                      "esq": (-1300, 0, 1), "dir": (1300, 0, 1)}[modo]
    return [(0, cx + dx, cy + dy, w * escala, h * escala, 0),
            (ent, cx, cy, w * 1.05, h * 1.05, 1),         # passa um pouco do ponto e assenta
            (ent + 3, cx, cy, w, h, 1),
            (dur - sai, cx, cy, w, h, 1),
            (dur - 1, cx, cy, w * 0.85, h * 0.85, 0)]


class Quadro:
    """Monta a timeline com `timeline_*`, `title_add` e `keyframe_set`, e escolhe as trilhas."""

    def __init__(self, s):
        self.s = s
        self.chamadas = 0
        self.ordem = []     # trilhas de baixo para cima
        self.usos = {}      # track_id -> [(início, fim)]
        self.formas = {}    # cor -> producer de um bloco cheio

    def c(self, ferramenta, args):
        self.chamadas += 1
        r = self.s.chamar(ferramenta, args)
        assert r.get("ok"), (ferramenta, args, r)
        return r

    def nova_camada(self, nome):
        tid = self.c("timeline_add_track", {"kind": "video"})["track_id"]
        self.c("track_set_name", {"track_id": tid, "name": nome})
        self.ordem.append(tid)
        self.usos[tid] = []
        return tid

    def camada(self, ini, fim):
        """Trilha que fica por cima de tudo o que se sobrepõe no tempo, sem criar trilha à toa."""
        cruza = lambda tid: any(a < fim and ini < b for a, b in self.usos[tid])
        mais_alta = max((i for i, tid in enumerate(self.ordem) if cruza(tid)), default=-1)
        if mais_alta + 1 < len(self.ordem):
            return self.ordem[mais_alta + 1]
        return self.nova_camada(f"Camada {len(self.ordem) + 1}")

    @staticmethod
    def retangulo(k):
        _, cx, cy, w, h, o = k[:6]
        return round(W / 2 + cx - w / 2, 2), round(H / 2 + cy - h / 2, 2), round(w, 2), round(h, 2), o

    def colocar(self, prod, ini, dur, kfs, camada=None):
        tid = camada if camada is not None else self.camada(ini, ini + dur)
        self.usos[tid].append((ini, ini + dur))
        ref = self.c("timeline_add_clip", {"track_id": tid, "producer_id": prod, "in_frame": 0,
                                           "out_frame": dur - 1, "position": ini})["clip_ref"]
        x, y, w, h, _ = self.retangulo(kfs[0])
        f = self.c("clip_set_transform", {"clip_ref": ref, "x": x, "y": y, "width": w, "height": h})["filter_id"]
        # sem `distort` o qtblend mantém a proporção do quadro dentro do retângulo
        self.c("filter_set_param", {"clip_ref": ref, "filter_id": f, "param": "distort", "value": "1"})
        self.c("filter_set_param", {"clip_ref": ref, "filter_id": f, "param": "rotate_center", "value": "1"})
        for k in kfs:
            valor = " ".join(str(v) for v in self.retangulo(k))
            self.c("keyframe_set", {"clip_ref": ref, "filter_id": f, "param": "rect", "frame": int(k[0]),
                                    "value": valor})
        for k in kfs:
            if len(k) > 6:
                self.c("keyframe_set", {"clip_ref": ref, "filter_id": f, "param": "rotation", "frame": int(k[0]),
                                        "value": str(k[6])})
        return ref

    def forma(self, hexa):
        """Um bloco de cor sólida: o glifo "████" em corpo grande preenche o quadro, e o retângulo o dimensiona."""
        if hexa not in self.formas:
            r = self.c("title_add", {"text": "████", "duration_frames": TOTAL, "font_family": FONTE,
                                     "font_size": 2400, "color": cor(hexa)})
            self.formas[hexa] = r["clip_id"]
        return self.formas[hexa]

    def bloco(self, hexa, cx, cy, w, h, ini, dur, modo="pop", ent=8, sai=6, kfs=None, camada=None):
        dur = int(dur)
        return self.colocar(self.forma(hexa), ini, dur, kfs or anima(cx, cy, w, h, dur, modo, ent, sai), camada)

    def texto(self, txt, tam, hexa, cx, cy, ini, dur, modo="pop", ent=8, sai=6, fonte=FONTE, kfs=None):
        dur = int(dur)
        largura = 0.6 * tam * len(txt)
        if largura > LARGURA_MAX:
            tam = tam * LARGURA_MAX / largura
        r = self.c("title_add", {"text": txt, "duration_frames": dur, "font_family": fonte,
                                 "font_size": max(8, int(tam)), "color": cor(hexa)})
        return self.colocar(r["clip_id"], ini, dur, kfs or anima(cx, cy, W, H, dur, modo, ent, sai))

    def bolha(self, txt, lado, fundo, letra, y, t0, L):
        """Mensagem de chat: fundo colorido e texto por cima, ambos chegando do mesmo lado."""
        tam = 40
        lw = 0.6 * tam * len(txt) + 70
        cx = (W / 2 - 60 - lw / 2) * (1 if lado == "dir" else -1)
        modo = "dir" if lado == "dir" else "esq"
        self.bloco(fundo, cx, y, lw, 110, t0, L, modo=modo)
        self.texto(txt, tam, letra, cx, y, t0 + 2, L - 2, modo=modo)

    def flash(self, ini, camada):
        """Um clarão cheio, de 10 quadros, no corte entre duas cenas."""
        self.bloco(LIMAO, 0, 0, W, H, ini, 10, camada=camada, kfs=[
            (0, 0, 0, W, H, 0), (1, 0, 0, W, H, 0.9), (4, 0, 0, W, H, 0.5), (9, 0, 0, W, H, 0)])

    def progresso(self, camada):
        """Barra fina na base que cresce até a largura do quadro durante o vídeo inteiro."""
        kfs = []
        for f in list(range(0, TOTAL, 90)) + [TOTAL - 1]:
            w = max(2, W * (f + 1) / TOTAL)
            kfs.append((f, -W / 2 + w / 2, 949, w, 10, 1))
        self.bloco(LIMAO, 0, 0, W, H, 0, TOTAL, kfs=kfs, camada=camada)


# =========================================================================================
def cena_gancho(q, t0, L):
    q.texto("uma IA", 150, BRANCO, 0, -330, t0, L, modo="esq", ent=6)
    q.texto("editou", 250, LIMAO, 0, -90, t0 + 10, L - 10, modo="estala", ent=6)
    q.texto("isso", 250, ROSA, 0, 130, t0 + 22, L - 22, modo="dir", ent=6)
    q.texto("sem encostar no mouse", 46, CINZA, 0, 330, t0 + 50, L - 50, modo="sobe", ent=10)


def cena_conversa(q, t0, L):
    mensagens = [
        ("crie um vídeo vertical para o TikTok", "dir", ROSA, PAPEL, -600, 6),
        ("✓ projeto 1080×1920 criado", "esq", CARVAO, LIMAO, -440, 60),
        ("✓ títulos e clipes na timeline", "esq", CARVAO, BRANCO, -280, 110),
        ("faz uma transição e um efeito", "dir", ROSA, PAPEL, -120, 160),
        ("✓ keyframes aplicados", "esq", CARVAO, LIMAO, 40, 210),
        ("✓ pronto para publicar", "esq", CARVAO, BRANCO, 200, 250),
    ]
    for txt, lado, fundo, letra, y, quando in mensagens:
        q.bolha(txt, lado, fundo, letra, y, t0 + quando, L - quando)


def cena_ferramentas(q, t0, L):
    q.texto("as ferramentas", 92, BRANCO, 0, -640, t0, L, modo="sobe")
    nomes = ["project_new", "timeline_add_clip", "title_add", "keyframe_set", "transition_add", "export_render"]
    for i, nome in enumerate(nomes):
        y = -470 + i * 170
        ini = t0 + 20 + i * 22
        dur = L - (ini - t0) - 4
        modo = "esq" if i % 2 == 0 else "dir"
        q.bloco(CARVAO, 0, y, 960, 120, ini, dur, modo=modo)
        q.texto(nome, 60, LIMAO, 0, y, ini + 2, dur - 2, modo=modo, fonte=FONTE_MONO)


def cena_numeros(q, t0, L):
    numeros = [("4.700", "chamadas ao MCP", LIMAO, -380), ("0", "cliques de mouse", ROSA, 0),
               ("1", "arquivo .kdenlive", CEU, 380)]
    for i, (numero, rotulo, cor_, y) in enumerate(numeros):
        ini = t0 + i * 50
        dur = L - i * 50
        q.texto(numero, 300, cor_, 0, y, ini, dur, modo="sobe")
        q.texto(rotulo, 56, BRANCO, 0, y + 190, ini + 8, dur - 8, modo="sobe")


def cena_timeline(q, t0, L):
    q.texto("a timeline", 84, BRANCO, 0, -640, t0, L, modo="desce")
    trilhas = [("V2", -420), ("V1", -260), ("A1", -100), ("A2", 60)]
    for i, (nome, y) in enumerate(trilhas):
        ini = t0 + i * 8
        q.bloco(CARVAO, 60, y, 880, 100, ini, L - (ini - t0) - 4, modo="esq", ent=6)
        q.texto(nome, 38, CINZA, -440, y, ini + 2, L - (ini - t0) - 6, modo="esq", ent=6, fonte=FONTE_MONO)
    clipes = [(0, -380, 300, ROSA), (0, -40, 360, ROSA), (1, -380, 460, LIMAO), (1, 120, 260, CEU),
              (2, -380, 640, CEU), (2, 300, 180, ROSA), (3, -200, 420, LIMAO)]
    for k, (trilha, x0, w, cor_) in enumerate(clipes):
        ini = t0 + 30 + k * 12
        q.bloco(cor_, x0 + w / 2, trilhas[trilha][1], w, 70, ini, L - (ini - t0) - 4, modo="dir", ent=6)
    # a cabeça de reprodução atravessa a timeline do começo ao fim da cena
    q.bloco(BRANCO, 0, 0, W, H, t0, L, kfs=[(0, -380, -180, 6, 560, 1), (L - 1, 500, -180, 6, 560, 1)])
    q.texto("timeline_add_clip · timeline_move_clip", 34, CINZA, 0, 300, t0 + 60, L - 60, fonte=FONTE_MONO,
            modo="sobe")
    q.texto("timeline_trim_clip · timeline_split_clip", 34, CINZA, 0, 350, t0 + 80, L - 80, fonte=FONTE_MONO,
            modo="sobe")


def cena_keyframes(q, t0, L):
    q.texto("keyframes", 84, BRANCO, 0, -640, t0, L, modo="sobe")
    q.bloco(LIMAO, 0, 0, 380, 380, t0, L, kfs=[
        (0, -260, 80, 120, 120, 0, 0), (20, -260, 80, 380, 380, 1, 0), (70, 0, -120, 480, 480, 1, 180),
        (110, 260, 120, 260, 260, 1, 360), (L - 30, 0, 0, 300, 300, 1, 540), (L - 1, 0, 0, 300, 300, 0, 540)])
    q.texto("posição · escala · rotação · opacidade", 36, CINZA, 0, 600, t0 + 40, L - 40, fonte=FONTE_MONO,
            modo="sobe")


def cena_final(q, t0, L):
    q.texto("KDENLIVE", 150, BRANCO, 0, -420, t0, L, modo="pop")
    q.texto("MCP PLUS", 150, LIMAO, 0, -250, t0 + 8, L - 8, modo="pop")
    q.texto("feito só por IA, pelo MCP", 46, CINZA, 0, -40, t0 + 24, L - 24, modo="sobe")
    q.texto("github.com/crom-org/kdenlive-mcp-plus", 40, BRANCO, 0, 120, t0 + 36, L - 36, fonte=FONTE_MONO,
            modo="sobe")
    q.texto("criado pela crom.run", 60, ROSA, 0, 260, t0 + 50, L - 50, modo="sobe")
    q.texto("feito com Claude Haiku 5.5", 40, CINZA, 0, 360, t0 + 60, L - 60, modo="sobe")
    q.texto("comenta o que você quer editar", 44, LIMAO, 0, 520, t0 + 80, L - 80, modo="sobe")


FUNCOES = {"gancho": cena_gancho, "conversa": cena_conversa, "ferramentas": cena_ferramentas,
           "numeros": cena_numeros, "timeline": cena_timeline, "keyframes": cena_keyframes, "final": cena_final}


def construir(s):
    q = Quadro(s)
    # fundo e rótulo de marca em toda a duração, na trilha de baixo
    q.bloco(PAPEL, 0, 0, W, H, 0, TOTAL, kfs=[(0, 0, 0, W, H, 1), (TOTAL - 1, 0, 0, W, H, 1)])
    q.texto("kdenlive-mcp-plus", 34, CINZA, 0, -840, 0, TOTAL, fonte=FONTE_MONO,
            kfs=[(0, 0, -840, W, H, 1), (TOTAL - 1, 0, -840, W, H, 1)])
    for nome, L in CENAS:
        FUNCOES[nome](q, INICIO[nome], L)
        print(f"  cena '{nome}': inicio {INICIO[nome]}, {L} quadros — {q.chamadas} chamadas, {len(q.ordem)} trilhas",
              flush=True)
    # por cima de tudo: o clarão em cada corte e a barra de progresso
    clarao = q.nova_camada("Clarões")
    for t in INICIO.values():
        q.flash(t, clarao)
    q.progresso(q.nova_camada("Progresso"))
    return q


if __name__ == "__main__":
    if len(sys.argv) < 2:
        sys.exit(__doc__)
    alvo = sys.argv[1]
    if os.path.exists(alvo):
        sys.exit(f"já existe, não sobrescrevo: {alvo}")
    binario = sys.argv[2] if len(sys.argv) > 2 else os.path.join(
        os.path.dirname(os.path.abspath(__file__)), "..", "..", "target", "release", "kdenlive-mcp-plus")
    s = Servidor(binario)
    r = s.chamar("project_new", {"profile": PERFIL})
    assert r.get("ok"), r
    t = time.time()
    q = construir(s)
    print(f"construído em {time.time() - t:.0f}s: {q.chamadas} chamadas MCP")
    print(s.chamar("project_save", {"path": alvo}))
    print(s.chamar("project_validate", {}))
    info = s.chamar("timeline_get_info", {})
    print("duração (quadros):", info["duracao_quadros"], "->", info["duracao_quadros"] / FPS, "s; trilhas:",
          len(info["trilhas"]))
    s.fechar()
