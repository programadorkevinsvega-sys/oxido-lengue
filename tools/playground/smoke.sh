#!/usr/bin/env bash
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
oxido_bin="${OXIDO_BIN:-$root/target/debug/oxido}"
playground_bin="${PLAYGROUND_BIN:-$root/target/debug/oxido-playground}"
bind="${PLAYGROUND_BIND:-127.0.0.1:8787}"
listener_port="${PLAYGROUND_LISTENER_PORT:-8790}"

for binary in "$oxido_bin" "$playground_bin"; do
  if [[ ! -x "$binary" ]]; then
    echo "falta el binario $binary; ejecuta cargo build -p oxido-cli -p oxido-playground" >&2
    exit 1
  fi
done

tmp="$(mktemp -d)"
server_pid=""
listener_pid=""
cleanup() {
  if [[ -n "$server_pid" ]]; then
    kill "$server_pid" 2>/dev/null || true
  fi
  if [[ -n "$listener_pid" ]]; then
    kill "$listener_pid" 2>/dev/null || true
  fi
  rm -rf "$tmp"
}
trap cleanup EXIT

python3 -m http.server "$listener_port" --bind 0.0.0.0 >"$tmp/listener.log" 2>&1 &
listener_pid=$!

host_ip="$(ip -4 route get 1.1.1.1 2>/dev/null | awk '{for (i = 1; i <= NF; i++) if ($i == "src") { print $(i + 1); exit }}')"
if [[ -z "$host_ip" ]]; then
  host_ip="$(hostname -I | awk '{print $1}')"
fi
if [[ -z "$host_ip" ]]; then
  echo "no se pudo determinar la IP del host" >&2
  exit 1
fi

# La red exterior debe funcionar antes de comprobar que el sandbox no la alcanza.
listener_ready=0
for _ in $(seq 1 50); do
  if curl -fsS --max-time 2 "http://$host_ip:$listener_port/" >/dev/null 2>&1; then
    listener_ready=1
    break
  fi
  sleep 0.1
done
if [[ "$listener_ready" -ne 1 ]]; then
  echo "el listener exterior no responde en $host_ip:$listener_port" >&2
  cat "$tmp/listener.log" >&2 || true
  exit 1
fi

"$playground_bin" --bind "$bind" --oxido "$oxido_bin" &
server_pid=$!

ready=0
for _ in $(seq 1 100); do
  if curl -fsS --max-time 2 "http://$bind/salud" >/dev/null 2>&1; then
    ready=1
    break
  fi
  sleep 0.1
done
if [[ "$ready" -ne 1 ]]; then
  echo "el playground no respondió en $bind" >&2
  exit 1
fi
curl -fsS --max-time 2 "http://$bind/" >/dev/null

python3 - "$host_ip" "$listener_port" "$tmp" <<'PY'
import json
import pathlib
import sys

host, port, directory = sys.argv[1], sys.argv[2], pathlib.Path(sys.argv[3])


def write(name, source):
    (directory / name).write_text(json.dumps({"fuente": source}), encoding="utf-8")


write(
    "hello.json",
    """sintaxis indentacion
funcion main():
    imprimir!("hola desde el sandbox")
""",
)
write(
    "net.json",
    f"""sintaxis indentacion
usar std::net::TcpStream

funcion main():
    si TcpStream::connect("{host}:{port}").is_ok():
        imprimir!("RED ACCESIBLE")
    sino:
        imprimir!("SIN RED")
""",
)
write(
    "path.json",
    """sintaxis indentacion
funcion main():
    si std::fs::metadata("/etc/hostname").is_ok():
        imprimir!("VISIBLE")
    sino:
        imprimir!("OCULTO")
""",
)
write(
    "timeout.json",
    """sintaxis indentacion
funcion main():
    std::thread::sleep(std::time::Duration::from_secs(30))
    imprimir!("no debería llegar")
""",
)
PY

post() {
  curl -sS --max-time 30 -H 'Content-Type: application/json' \
    --data-binary "@$1" "http://$bind/api/ejecutar"
}

check() {
  local name="$1"
  local response="$2"
  local program="$3"
  printf '%s\n' "$response"
  if ! printf '%s' "$response" | python3 -c "$program"; then
    echo "prueba fallida: $name" >&2
    exit 1
  fi
}

check "hola" "$(post "$tmp/hello.json")" \
  'import json, sys; d = json.load(sys.stdin); assert d["estado"] == "ok", d; assert d["codigo"] == 0, d; assert "hola desde el sandbox" in d["salida"], d'
check "red" "$(post "$tmp/net.json")" \
  'import json, sys; d = json.load(sys.stdin); assert d["estado"] == "ok", d; assert d["salida"].strip() == "SIN RED", d'
check "rutas" "$(post "$tmp/path.json")" \
  'import json, sys; d = json.load(sys.stdin); assert d["estado"] == "ok", d; assert d["salida"].strip() == "OCULTO", d'
check "tiempo" "$(post "$tmp/timeout.json")" \
  'import json, sys; d = json.load(sys.stdin); assert d["estado"] == "tiempo_agotado", d; assert d["tiempo_agotado"] is True, d'

status="$(curl -sS --max-time 10 -o /dev/null -w '%{http_code}' \
  --data-binary "@$tmp/hello.json" "http://$bind/api/ejecutar")"
if [[ "$status" != "415" ]]; then
  echo "se esperaba 415 sin content-type y llegó $status" >&2
  exit 1
fi

echo "Smoke del sandbox correcto: ejecución, red, rutas y plazo de pared."
