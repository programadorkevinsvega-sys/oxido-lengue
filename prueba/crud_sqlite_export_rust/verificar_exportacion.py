#!/usr/bin/env python3
"""Ejecuta el cliente Rust exportado a través de reinicios del servidor."""

from __future__ import annotations

import http.client
import os
from pathlib import Path
import signal
import socket
import subprocess
import sys
import time


ROOT = Path(__file__).resolve().parents[2]
TARGET = Path(os.environ.get("CARGO_TARGET_DIR", "/tmp/crud-sqlite-export-target")).resolve()
SERVER = TARGET / "release/crud-api-exportado"
CLIENT = TARGET / "release/verificar-persistencia-exportado"
ADDRESS = ("127.0.0.1", 38212)
DATABASE = "/tmp/crud-sqlite-exportado.sqlite"
CHECKPOINT = "/tmp/crud-sqlite-exportado-instance.txt"


def port_is_open() -> bool:
    with socket.socket() as connection:
        connection.settimeout(0.2)
        return connection.connect_ex(ADDRESS) == 0


def stop(process: subprocess.Popen[bytes] | None) -> None:
    if process is None or process.poll() is not None:
        return
    process.send_signal(signal.SIGTERM)
    try:
        process.wait(timeout=10)
    except subprocess.TimeoutExpired:
        process.kill()
        process.wait()


def start(reset: bool) -> subprocess.Popen[bytes]:
    if port_is_open():
        raise RuntimeError("el puerto 38212 está ocupado; no se ejecutará la prueba")
    environment = os.environ.copy()
    environment["CRUD_SQLITE_PATH"] = DATABASE
    environment["CRUD_ADDR"] = f"{ADDRESS[0]}:{ADDRESS[1]}"
    if reset:
        environment["CRUD_RESET_DB"] = "1"
    else:
        environment.pop("CRUD_RESET_DB", None)
    process = subprocess.Popen([str(SERVER)], cwd=ROOT, env=environment)
    deadline = time.monotonic() + 10
    while time.monotonic() < deadline:
        if process.poll() is not None:
            raise RuntimeError(f"el servidor exportado terminó con estado {process.returncode}")
        try:
            connection = http.client.HTTPConnection(*ADDRESS, timeout=2)
            connection.request("GET", "/__test/instance")
            response = connection.getresponse()
            response.read()
            connection.close()
            if response.status == 200:
                return process
        except OSError:
            time.sleep(0.1)
    stop(process)
    raise TimeoutError("el servidor exportado no abrió el puerto de prueba")


def run_client(mode: str) -> None:
    environment = os.environ.copy()
    environment["CRUD_TEST_MODE"] = mode
    environment["CRUD_TEST_ADDR"] = f"{ADDRESS[0]}:{ADDRESS[1]}"
    environment["CRUD_TEST_STATE_PATH"] = CHECKPOINT
    print(f"\n$ cliente Rust exportado: modo {mode}", flush=True)
    subprocess.run([str(CLIENT)], cwd=ROOT, env=environment, check=True)


def main() -> int:
    if not SERVER.is_file() or not CLIENT.is_file():
        raise RuntimeError(f"faltan binarios; compila el proyecto con --manifest-path {ROOT / 'prueba/crud_sqlite_export_rust/Cargo.toml'}")
    server: subprocess.Popen[bytes] | None = None
    try:
        server = start(reset=True)
        run_client("preparar")
        stop(server)

        server = start(reset=False)
        run_client("reinicio")
        run_client("mutaciones")
        stop(server)

        server = start(reset=False)
        run_client("final")
        stop(server)
        server = None
        if port_is_open():
            raise RuntimeError("el puerto siguió ocupado después de la prueba")
        print("\nExportación compilada y flujo de persistencia verificado.", flush=True)
        return 0
    finally:
        stop(server)


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except (OSError, RuntimeError, subprocess.CalledProcessError, TimeoutError) as error:
        print(f"Error: {error}", file=sys.stderr)
        raise SystemExit(1)
