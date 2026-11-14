#!/usr/bin/env bash
set -euo pipefail

prefix=${OXIDO_VSCODE_PREFIX:-}
if [[ -n "$prefix" ]]; then
    vsix=${prefix%/}/usr/share/oxido-vscode/extension.vsix
else
    vsix=/usr/share/oxido-vscode/extension.vsix
fi
usage() {
    printf 'Uso: oxido-vscode-instalar [--code|--insiders|--editor RUTA]\n' >&2
}

editor=
while (($# > 0)); do
    case "$1" in
        --code) editor=code; shift ;;
        --insiders) editor=code-insiders; shift ;;
        --editor)
            [[ $# -ge 2 && -n "$2" ]] || { usage; exit 2; }
            editor=$2
            shift 2
            ;;
        -h|--help) usage; exit 0 ;;
        *) usage; printf 'ERROR: opción desconocida: %s\n' "$1" >&2; exit 2 ;;
    esac
done

[[ -f "$vsix" && ! -L "$vsix" ]] || {
    printf 'ERROR: no se encontró el VSIX instalado en %s.\n' "$vsix" >&2
    exit 1
}
if [[ -z "$editor" ]]; then
    if command -v code >/dev/null 2>&1; then
        editor=$(command -v code)
    elif command -v code-insiders >/dev/null 2>&1; then
        editor=$(command -v code-insiders)
    else
        printf 'ERROR: instale VS Code o VS Code Insiders antes de instalar la extensión.\n' >&2
        exit 1
    fi
fi
command -v -- "$editor" >/dev/null 2>&1 || [[ -x "$editor" ]] || {
    printf 'ERROR: no se encontró un ejecutable de editor: %s\n' "$editor" >&2
    exit 1
}
exec "$editor" --install-extension "$vsix"
