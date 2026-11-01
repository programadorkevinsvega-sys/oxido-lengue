#!/usr/bin/env bash
# Comprueba que el producto no incorpore SDK de telemetría ni envíe datos por defecto.
# Es una barrera estática: no sustituye una auditoría de red en ejecución.
set -euo pipefail

status=0

fail() {
  printf '%s\n' "$1" >&2
  status=1
}

# Dependencias declaradas por el workspace, la extensión y el portal.
manifests=(
  Cargo.toml
  Cargo.lock
  editors/vscode-oxido/package.json
  editors/vscode-oxido/package-lock.json
  website/package.json
  website/package-lock.json
)

telemetry_dependencies='sentry|posthog|mixpanel|amplitude|@segment/|datadog|newrelic|opentelemetry|plausible|google-analytics|@vercel/analytics|prom-client|applicationinsights'

for manifest in "${manifests[@]}"; do
  [[ -f "$manifest" ]] || continue
  if grep -nEi "$telemetry_dependencies" "$manifest" >/dev/null; then
    fail "dependencia de telemetría detectada en $manifest"
  fi
done

# Código propio: SDK, balizas y endpoints de analítica.
source_roots=(crates editors/vscode-oxido/src)
telemetry_code='sentry|posthog|mixpanel|amplitude|segment\.io|datadog|newrelic|opentelemetry|plausible\.io|google-analytics|gtag\(|@vercel/analytics|applicationinsights'

for root in "${source_roots[@]}"; do
  [[ -d "$root" ]] || continue
  matches="$(grep -RInEi "$telemetry_code" "$root" --include='*.rs' --include='*.ts' --include='*.js' || true)"
  if [[ -n "$matches" ]]; then
    while IFS= read -r match; do
      printf 'telemetría potencial: %s\n' "$match" >&2
    done <<<"$matches"
    status=1
  fi
done

if [[ $status -ne 0 ]]; then
  printf 'La telemetría remota está desactivada por defecto; elimina el SDK o documenta una decisión explícita.\n' >&2
  exit 1
fi

printf 'Telemetría: sin SDK ni endpoints de analítica en dependencias y código propio.\n'
