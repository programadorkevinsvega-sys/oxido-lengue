#!/usr/bin/env bash
# Comprueba que los informes exportables no filtren secretos ni rutas absolutas.
# Uso: tools/report-hygiene.sh <archivo-o-directorio>...
set -euo pipefail

if [[ $# -eq 0 ]]; then
  printf 'uso: %s <archivo-o-directorio>...\n' "$0" >&2
  exit 2
fi

status=0

check_file() {
  local file="$1"
  local matches

  # -I evita inspeccionar binarios; -E agrupa los patrones de riesgo.
  matches="$(
    grep -nIE \
      -e '/home/[A-Za-z0-9._-]+' \
      -e '/Users/[A-Za-z0-9._-]+' \
      -e '/root/' \
      -e '/tmp/' \
      -e '/github/workspace' \
      -e '[A-Za-z]:\\Users\\' \
      -e '[A-Za-z]:\\a\\' \
      -e 'ghp_[A-Za-z0-9]{20,}' \
      -e 'github_pat_[A-Za-z0-9_]{20,}' \
      -e 'AKIA[0-9A-Z]{16}' \
      -e '-----BEGIN [A-Z ]*PRIVATE KEY-----' \
      -e 'xox[baprs]-[A-Za-z0-9-]+' \
      -e 'GITHUB_TOKEN[[:space:]]*=' \
      -e 'ACTIONS_RUNTIME_TOKEN' \
      -e 'CARGO_REGISTRY_TOKEN' \
      -e '(password|contrasena|contraseña|token|secret)[[:space:]]*[:=][[:space:]]*[^[:space:]]{8,}' \
      "$file" 2>/dev/null || true
  )"

  if [[ -n "$matches" ]]; then
    while IFS= read -r match; do
      local line_number="${match%%:*}"
      # Nunca se imprime el valor coincidente: podría ser el propio secreto.
      printf 'fuga potencial en %s:%s (se omite el valor)\n' "$file" "$line_number" >&2
    done <<<"$matches"
    status=1
  fi
}

while [[ $# -gt 0 ]]; do
  target="$1"
  shift

  if [[ -d "$target" ]]; then
    while IFS= read -r -d '' file; do
      check_file "$file"
    done < <(find "$target" -type f -print0)
  elif [[ -f "$target" ]]; then
    check_file "$target"
  else
    printf 'no existe: %s\n' "$target" >&2
    status=1
  fi
done

if [[ $status -ne 0 ]]; then
  printf 'Los informes deben normalizar rutas y excluir credenciales antes de publicarse.\n' >&2
  exit 1
fi

printf 'Higiene de informes: sin secretos ni rutas absolutas detectadas.\n'
