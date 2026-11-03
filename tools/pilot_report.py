#!/usr/bin/env python3
"""Agrega datos anonimizados de un piloto educativo de Óxido.

No recoge datos, no envía nada por red y no inventa resultados. Solo lee dos
CSV locales, valida que no contengan datos personales evidentes y escribe un
informe descriptivo en Markdown.

Uso:
    python3 tools/pilot_report.py \
        --participantes docs/pilotos/participantes-ejemplo.csv \
        --ejercicios docs/pilotos/ejercicios-ejemplo.csv \
        --salida target/informe-piloto.md \
        --sintetico
"""

from __future__ import annotations

import argparse
import csv
import re
import statistics
import sys
from collections import Counter, defaultdict
from dataclasses import dataclass
from pathlib import Path

CODE_PATTERN = re.compile(r"^[A-Z]{1,2}-[0-9]{3}$")
DIAGNOSTIC_PATTERN = re.compile(r"^(OX[0-9]{4}|[EW][0-9]{4})$")
PRIVATE_MARKERS = ("/", "\\", "@", "C:", "file:")


class PilotDataError(ValueError):
    """El CSV no cumple el contrato o contiene datos no admitidos."""


@dataclass(frozen=True)
class Participant:
    code: str
    group: str
    first_run_seconds: int | None
    clarity: int | None
    pre_points: int | None
    post_points: int | None


@dataclass(frozen=True)
class ExerciseRow:
    code: str
    exercise: str
    completed: bool
    seconds: int | None
    diagnostics: tuple[str, ...]
    help_count: int


def read_rows(path: Path, expected: set[str]) -> list[dict[str, str]]:
    if not path.is_file():
        raise PilotDataError(f"no existe el archivo {path}")
    with path.open(encoding="utf-8", newline="") as handle:
        reader = csv.DictReader(handle)
        if reader.fieldnames is None:
            raise PilotDataError(f"{path} no tiene cabecera")
        missing = expected - set(reader.fieldnames)
        if missing:
            raise PilotDataError(f"{path} no tiene las columnas: {', '.join(sorted(missing))}")
        rows = []
        for number, row in enumerate(reader, start=2):
            for column, value in row.items():
                value = (value or "").strip()
                if any(marker in value for marker in PRIVATE_MARKERS):
                    raise PilotDataError(
                        f"{path}:{number}: la columna {column!r} parece contener una ruta, "
                        "un correo o un identificador de archivo; usa solo códigos anónimos"
                    )
                row[column] = value
            rows.append(row)
    return rows


def parse_optional_int(value: str, field: str, minimum: int = 0, maximum: int | None = None) -> int | None:
    if value == "":
        return None
    try:
        number = int(value)
    except ValueError as error:
        raise PilotDataError(f"{field}: se esperaba un entero y llegó {value!r}") from error
    if number < minimum:
        raise PilotDataError(f"{field}: {number} es menor que {minimum}")
    if maximum is not None and number > maximum:
        raise PilotDataError(f"{field}: {number} es mayor que {maximum}")
    return number


def check_code(value: str, field: str) -> str:
    if not CODE_PATTERN.match(value):
        raise PilotDataError(
            f"{field}: el código {value!r} no cumple el formato anónimo P-001 o C-001"
        )
    return value


def parse_participants(path: Path) -> list[Participant]:
    rows = read_rows(
        path,
        {"codigo", "grupo", "tiempo_primera_ejecucion_seg", "claridad", "pre_puntos", "post_puntos"},
    )
    participants = []
    seen: set[str] = set()
    for row in rows:
        code = check_code(row["codigo"], f"{path}:codigo")
        if code in seen:
            raise PilotDataError(f"{path}: el código {code} está repetido")
        seen.add(code)
        group = row["grupo"].lower()
        if group not in {"piloto", "control"}:
            raise PilotDataError(f"{path}:{code}: el grupo debe ser piloto o control")
        participants.append(
            Participant(
                code=code,
                group=group,
                first_run_seconds=parse_optional_int(
                    row["tiempo_primera_ejecucion_seg"], f"{path}:{code}:tiempo_primera_ejecucion_seg"
                ),
                clarity=parse_optional_int(row["claridad"], f"{path}:{code}:claridad", 1, 5),
                pre_points=parse_optional_int(row["pre_puntos"], f"{path}:{code}:pre_puntos", 0),
                post_points=parse_optional_int(row["post_puntos"], f"{path}:{code}:post_puntos", 0),
            )
        )
    if not participants:
        raise PilotDataError(f"{path} no contiene participantes")
    return participants


def parse_exercises(path: Path, participants: set[str]) -> list[ExerciseRow]:
    rows = read_rows(
        path,
        {"codigo", "ejercicio", "completado", "segundos", "diagnosticos", "ayudas"},
    )
    exercises = []
    for row in rows:
        code = check_code(row["codigo"], f"{path}:codigo")
        if code not in participants:
            raise PilotDataError(f"{path}: el código {code} no existe en participantes")
        completed = row["completado"].lower()
        if completed not in {"si", "sí", "no"}:
            raise PilotDataError(f"{path}:{code}: completado debe ser si o no")
        exercise = row["ejercicio"].strip()
        if not re.match(r"^[1-9][0-9]?$", exercise):
            raise PilotDataError(f"{path}:{code}: el ejercicio debe ser un número del 1 al 99")
        diagnostics = tuple(part for part in re.split(r"[|, ]+", row["diagnosticos"]) if part)
        for diagnostic in diagnostics:
            if not DIAGNOSTIC_PATTERN.match(diagnostic):
                raise PilotDataError(
                    f"{path}:{code}: el diagnóstico {diagnostic!r} debe ser un código OX####, E#### o W####"
                )
        exercises.append(
            ExerciseRow(
                code=code,
                exercise=exercise,
                completed=completed in {"si", "sí"},
                seconds=parse_optional_int(row["segundos"], f"{path}:{code}:segundos"),
                diagnostics=diagnostics,
                help_count=parse_optional_int(row["ayudas"], f"{path}:{code}:ayudas") or 0,
            )
        )
    if not exercises:
        raise PilotDataError(f"{path} no contiene ejercicios")
    return exercises


def summary(values: list[int]) -> str:
    if not values:
        return "sin datos"
    if len(values) == 1:
        return f"mediana {values[0]}, rango {values[0]}–{values[0]}"
    return f"mediana {statistics.median(values):g}, rango {min(values)}–{max(values)}"


def format_percentage(done: int, total: int) -> str:
    return f"{done}/{total} ({100 * done / total:.0f}%)" if total else "0/0"


def build_report(participants: list[Participant], exercises: list[ExerciseRow], synthetic: bool) -> str:
    by_group: dict[str, list[Participant]] = defaultdict(list)
    for participant in participants:
        by_group[participant.group].append(participant)

    lines = ["# Informe de piloto educativo", ""]
    if synthetic:
        lines += [
            "> **Datos sintéticos de ejemplo.** Este informe no contiene resultados de estudiantes "
            "y no debe presentarse como evidencia educativa.",
            "",
        ]
    lines += [
        "Resultados descriptivos de un piloto local. No demuestran eficacia, no comparan causalmente "
        "los grupos y no se generalizan a otras aulas.",
        "",
        "## Participación",
        "",
        "| Grupo | Participantes |",
        "|---|---|",
    ]
    for group in ("piloto", "control"):
        lines.append(f"| {group} | {len(by_group.get(group, []))} |")

    lines += ["", "## Finalización por ejercicio", "", "| Ejercicio | Grupo | Completados |", "|---|---|---|"]
    for exercise in sorted({row.exercise for row in exercises}, key=int):
        for group in ("piloto", "control"):
            group_codes = {p.code for p in by_group.get(group, [])}
            rows = [row for row in exercises if row.exercise == exercise and row.code in group_codes]
            done = sum(1 for row in rows if row.completed)
            lines.append(f"| {exercise} | {group} | {format_percentage(done, len(rows))} |")

    lines += ["", "## Tiempo hasta la primera ejecución", ""]
    for group in ("piloto", "control"):
        values = [
            p.first_run_seconds for p in by_group.get(group, []) if p.first_run_seconds is not None
        ]
        lines.append(f"- **{group}:** {summary(values)} segundos ({len(values)} con dato).")

    diagnostics = Counter(
        diagnostic for row in exercises for diagnostic in row.diagnostics
    )
    lines += ["", "## Diagnósticos observados", ""]
    if diagnostics:
        lines += ["| Código | Veces |", "|---|---|"]
        for code, count in diagnostics.most_common():
            lines.append(f"| `{code}` | {count} |")
    else:
        lines.append("Sin diagnósticos registrados.")

    lines += ["", "## Ayudas del docente", ""]
    for group in ("piloto", "control"):
        group_codes = {p.code for p in by_group.get(group, [])}
        total = sum(row.help_count for row in exercises if row.code in group_codes)
        lines.append(f"- **{group}:** {total} ayudas registradas.")

    lines += ["", "## Claridad percibida (1–5)", ""]
    for group in ("piloto", "control"):
        values = [p.clarity for p in by_group.get(group, []) if p.clarity is not None]
        lines.append(f"- **{group}:** {summary(values)} ({len(values)} respuestas).")

    lines += ["", "## Cambio en la prueba conceptual", ""]
    paired = [
        p for p in participants if p.pre_points is not None and p.post_points is not None
    ]
    if paired:
        differences = [p.post_points - p.pre_points for p in paired]
        lines.append(
            f"- Participantes con pre y post: {len(paired)}. "
            f"Diferencia individual: {summary(differences)} puntos."
        )
        lines.append("- Es una señal descriptiva, no una prueba causal.")
    else:
        lines.append("No hay participantes con ambas mediciones.")

    lines += [
        "",
        "## Límites",
        "",
        "- La muestra es pequeña y voluntaria; no representa a otros centros.",
        "- No se calculan pruebas de significación ni efectos causales.",
        "- Los datos faltantes no se imputan.",
        "- Cualquier resultado negativo o ambiguo debe publicarse junto con los positivos.",
        "",
    ]
    return "\n".join(lines)


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description="Agrega datos anonimizados de un piloto educativo.")
    parser.add_argument("--participantes", type=Path, required=True)
    parser.add_argument("--ejercicios", type=Path, required=True)
    parser.add_argument("--salida", type=Path, required=True)
    parser.add_argument(
        "--sintetico",
        action="store_true",
        help="marca el informe como datos sintéticos de ejemplo",
    )
    args = parser.parse_args(argv)

    try:
        participants = parse_participants(args.participantes)
        exercises = parse_exercises(args.ejercicios, {p.code for p in participants})
        report = build_report(participants, exercises, args.sintetico)
    except PilotDataError as error:
        print(f"piloto: {error}", file=sys.stderr)
        return 2

    args.salida.parent.mkdir(parents=True, exist_ok=True)
    args.salida.write_text(report, encoding="utf-8")
    print(f"Informe escrito en {args.salida}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
