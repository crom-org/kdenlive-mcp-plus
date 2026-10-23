"""Cliente MCP mínimo por stdio, só para o exemplo `construir_demo.py`.

Fala JSON-RPC (uma mensagem por linha) com `kdenlive-mcp-plus mcp`, como um
cliente MCP de verdade (o Claude Desktop, por exemplo) faria.
"""
import json
import os
import subprocess


class Servidor:
    def __init__(self, binario, env=None, stderr=None):
        """`env`: variáveis a acrescentar (ex. KDENLIVE_MCP_AUTORELOAD=1); `stderr`: arquivo de log."""
        self.p = subprocess.Popen([binario, "mcp"], stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                                  stderr=open(stderr, "w") if stderr else subprocess.DEVNULL, text=True, bufsize=1,
                                  env={**os.environ, **(env or {})})
        self.n = 0
        r = self._rpc("initialize", {"protocolVersion": "2025-06-18", "capabilities": {},
                                      "clientInfo": {"name": "exemplo-demo", "version": "0"}})
        self.info = r["result"]["serverInfo"]
        self._enviar({"jsonrpc": "2.0", "method": "notifications/initialized"})

    def _enviar(self, msg):
        self.p.stdin.write(json.dumps(msg) + "\n")
        self.p.stdin.flush()

    def _rpc(self, metodo, params):
        self.n += 1
        self._enviar({"jsonrpc": "2.0", "id": self.n, "method": metodo, "params": params})
        while True:
            linha = self.p.stdout.readline()
            if not linha:
                raise RuntimeError("o servidor encerrou")
            msg = json.loads(linha)
            if msg.get("id") == self.n:
                return msg

    def chamar(self, ferramenta, args=None):
        """Chama uma ferramenta e devolve o objeto `{"ok": ..., ...}` que ela responde."""
        r = self._rpc("tools/call", {"name": ferramenta, "arguments": args or {}})
        if "error" in r:
            raise RuntimeError(f"{ferramenta}: {r['error']}")
        res = r["result"]
        if res.get("structuredContent") is not None:
            return res["structuredContent"]
        return json.loads(res["content"][0]["text"])

    def fechar(self):
        self.p.stdin.close()
        self.p.wait(timeout=10)
