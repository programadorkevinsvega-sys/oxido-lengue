#!/usr/bin/env python3
"""Compara en secuencia ambos servidores con el mismo cliente release."""

from __future__ import annotations

import argparse
import http.client
import json
import os
from pathlib import Path
import signal
import socket
import subprocess
import sys
import time


ROOT = Path(__file__).resolve().parents[2]
OX_FILE = ROOT / "prueba/crud_sqlite/crud_api_persistencia.ox"
RUST_MANIFEST = ROOT / "prueba/crud_sqlite_rust/Cargo.toml"
ADDRESS = ("127.0.0.1", 38212)


def port_is_open() -> bool:
    with socket.socket() as connection:
        connection.settimeout(0.2)
        return connection.connect_ex(ADDRESS) == 0


def request(path: str) -> tuple[int, bytes]:
    connection = http.client.HTTPConnection(*ADDRESS, timeout=5)
    try:
        connection.request("GET", path)
        response = connection.getresponse()
        return response.status, response.read()
    finally:
        connection.close()


def wait_for_server(process: subprocess.Popen[bytes]) -> int:
    deadline = time.monotonic() + 30
    while time.monotonic() < deadline:
        if process.poll() is not None:
            raise RuntimeError(f"el lanzador del servidor terminó con {process.returncode}")
        try:
            status, body = request("/__test/instance")
            if status == 200:
                return int(json.loads(body)["pid"])
        except (OSError, ValueError, json.JSONDecodeError):
            time.sleep(0.1)
    raise TimeoutError("el servidor no abrió el puerto de prueba en 30 segundos")


def warm_up() -> None:
    for _ in range(10):
        status, _ = request("/productos")
        if status != 200:
            raise RuntimeError(f"el calentamiento devolvió HTTP {status}")


def stop_server(process: subprocess.Popen[bytes], server_pid: int) -> None:
    try:
        os.kill(server_pid, signal.SIGTERM)
    except ProcessLookupError:
        pass
    try:
        process.wait(timeout=10)
    except subprocess.TimeoutExpired:
        process.terminate()
        try:
            process.wait(timeout=5)
        except subprocess.TimeoutExpired:
            process.kill()
            process.wait()
    deadline = time.monotonic() + 5
    while port_is_open() and time.monotonic() < deadline:
        time.sleep(0.1)
    if port_is_open():
        raise RuntimeError("el puerto sigue ocupado; no se iniciará el otro servidor")


def start_and_measure(
    label: str,
    command: list[str],
    environment: dict[str, str],
    benchmark: Path,
    measure: bool = True,
) -> None:
    if port_is_open():
        raise RuntimeError("el puerto 38212 ya está ocupado; detén cualquier servidor antes de medir")
    print(f"\n=== {label}: iniciar ===", flush=True)
    capture_output = not measure
    process = subprocess.Popen(
        command,
        cwd=ROOT,
        env=environment,
        stdout=subprocess.PIPE if capture_output else None,
        stderr=subprocess.PIPE if capture_output else None,
        text=capture_output,
    )
    server_pid: int | None = None
    try:
        server_pid = wait_for_server(process)
        executable = Path(f"/proc/{server_pid}/exe").resolve()
        expected_profile = "/release/" in str(executable)
        if not expected_profile:
            raise RuntimeError(f"el servidor no usa artefacto release: {executable}")
        warm_up()
        print(
            f"Calentamiento: 10 consultas GET /productos; binario: {executable.name}",
            flush=True,
        )
        if measure:
            subprocess.run([str(benchmark)], cwd=ROOT, env=environment, check=True)
    finally:
        if server_pid is not None:
            stop_server(process, server_pid)
        elif process.poll() is None:
            process.terminate()
            process.wait(timeout=10)
        if capture_output:
            stdout, stderr = process.communicate()
            if server_pid is None:
                if stdout:
                    print(stdout, end="", file=sys.stderr)
                if stderr:
                    print(stderr, end="", file=sys.stderr)
    print(f"=== {label}: detenido; puerto libre ===", flush=True)


def run_command(command: list[str], environment: dict[str, str]) -> None:
    display_command = [
        str(Path(argument).relative_to(ROOT))
        if Path(argument).is_absolute() and Path(argument).is_relative_to(ROOT)
        else argument
        for argument in command
    ]
    print("$ " + " ".join(display_command), flush=True)
    result = subprocess.run(
        command,
        cwd=ROOT,
        env=environment,
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
        check=False,
    )
    if result.returncode != 0:
        output = result.stdout.decode("utf-8", errors="replace").replace(str(ROOT), ".")
        raise RuntimeError(
            f"el comando terminó con código {result.returncode}: {output.strip()}"
        )


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--rondas", type=int, default=5, help="pares de medición (predeterminado: 5)")
    parser.add_argument(
        "--solo-funcional",
        action="store_true",
        help="inicia ambos servidores secuencialmente y valida HTTP sin ejecutar cargas medidas",
    )
    args = parser.parse_args()
    if args.rondas < 1:
        parser.error("--rondas debe ser al menos 1")

    target_dir = Path(os.environ.get("CARGO_TARGET_DIR", "/tmp/crud-sqlite-release-target"))
    target_dir = target_dir.resolve()
    environment = os.environ.copy()
    environment["CARGO_TARGET_DIR"] = str(target_dir)
    environment["CRUD_SQLITE_PATH"] = "/tmp/crud-sqlite-comparacion.sqlite"
    environment["CRUD_RESET_DB"] = "1"
    benchmark = target_dir / "release/benchmark-rust"
    rust_server = target_dir / "release/crud-api-rust"
    oxido_cli = target_dir / "debug/oxido"
    oxido_server = target_dir / "release/oxido_generated"

    run_command(["cargo", "build", "--locked", "--offline", "-p", "oxido-cli"], environment)
    run_command(
        [
            "cargo",
            "build",
            "--release",
            "--locked",
            "--offline",
            "--manifest-path",
            str(RUST_MANIFEST),
            "--bins",
        ],
        environment,
    )
    if not benchmark.is_file() or not rust_server.is_file() or not oxido_cli.is_file():
        raise RuntimeError("faltan binarios compilados para la comparación")

    oxido_command = [
        str(oxido_cli),
        "ejecutar",
        str(OX_FILE),
        "--release",
        "--confiar-dependencias",
    ]
    rust_command = [str(rust_server)]

    # Primera ejecución: compila el Rust generado fuera de las mediciones.
    start_and_measure(
        "Óxido (compilación previa y calentamiento)",
        oxido_command,
        environment,
        benchmark,
        measure=False,
    )
    if not oxido_server.is_file():
        raise RuntimeError("Óxido no generó el binario release esperado")
    oxido_command = [str(oxido_server)]

    for round_number in range(args.rondas):
        order = (
            [("Óxido", oxido_command), ("Rust nativo", rust_command)]
            if round_number % 2 == 0
            else [("Rust nativo", rust_command), ("Óxido", oxido_command)]
        )
        print(f"\n######## RONDA {round_number + 1}/{args.rondas} ########", flush=True)
        for label, command in order:
            start_and_measure(
                label,
                command,
                environment,
                benchmark,
                measure=not args.solo_funcional,
            )
    if args.solo_funcional:
        print("Regresión HTTP funcional completada; no se midió rendimiento.", flush=True)
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except (OSError, RuntimeError, subprocess.CalledProcessError, TimeoutError) as error:
        print(f"Error: {error}", file=sys.stderr)
        raise SystemExit(1)
