import importlib.util
import sys
import tempfile
import unittest
from pathlib import Path

SCRIPT = Path(__file__).with_name("pilot_report.py")
SPEC = importlib.util.spec_from_file_location("pilot_report", SCRIPT)
pilot = importlib.util.module_from_spec(SPEC)
assert SPEC.loader is not None
sys.modules[SPEC.name] = pilot
SPEC.loader.exec_module(pilot)

PARTICIPANTS = """codigo,grupo,tiempo_primera_ejecucion_seg,claridad,pre_puntos,post_puntos
P-001,piloto,120,4,2,4
P-002,piloto,180,5,1,3
C-001,control,240,3,2,3
C-002,control,,2,,3
"""

EXERCISES = """codigo,ejercicio,completado,segundos,diagnosticos,ayudas
P-001,1,si,300,OX1001|E0308,1
P-001,2,si,420,OX2002,0
P-002,1,si,360,OX1001,2
P-002,2,no,600,OX1001,3
C-001,1,si,300,,0
C-001,2,si,480,E0308,2
C-002,1,no,540,OX2002,4
C-002,2,no,,,1
"""


class PilotReportTests(unittest.TestCase):
    def write(self, directory: Path, name: str, text: str) -> Path:
        path = directory / name
        path.write_text(text, encoding="utf-8")
        return path

    def parse(self, directory: Path, participants: str = PARTICIPANTS, exercises: str = EXERCISES):
        participant_path = self.write(directory, "participantes.csv", participants)
        exercise_path = self.write(directory, "ejercicios.csv", exercises)
        parsed_participants = pilot.parse_participants(participant_path)
        parsed_exercises = pilot.parse_exercises(
            exercise_path, {p.code for p in parsed_participants}
        )
        return parsed_participants, parsed_exercises

    def test_builds_descriptive_report_with_denominators(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            participants, exercises = self.parse(Path(temporary))
            report = pilot.build_report(participants, exercises, synthetic=True)
        self.assertIn("Datos sintéticos de ejemplo", report)
        self.assertIn("| piloto | 2 |", report)
        self.assertIn("| 2 | control | 1/2 (50%) |", report)
        self.assertIn("mediana 150", report)
        self.assertIn("`OX1001` | 3", report)
        self.assertIn("Participantes con pre y post: 3", report)
        self.assertIn("no una prueba causal", report)

    def test_rejects_email_or_paths(self) -> None:
        unsafe = PARTICIPANTS.replace("P-001", "ana@example.com")
        with tempfile.TemporaryDirectory() as temporary:
            path = self.write(Path(temporary), "participantes.csv", unsafe)
            with self.assertRaises(pilot.PilotDataError):
                pilot.parse_participants(path)

    def test_rejects_absolute_path(self) -> None:
        unsafe = EXERCISES.replace("OX1001|E0308", "/home/ana/OX1001")
        with tempfile.TemporaryDirectory() as temporary:
            participant_path = self.write(Path(temporary), "participantes.csv", PARTICIPANTS)
            exercise_path = self.write(Path(temporary), "ejercicios.csv", unsafe)
            participants = pilot.parse_participants(participant_path)
            with self.assertRaises(pilot.PilotDataError):
                pilot.parse_exercises(exercise_path, {p.code for p in participants})

    def test_rejects_free_text_diagnostics(self) -> None:
        unsafe = EXERCISES.replace("OX1001|E0308", "error de Ana")
        with tempfile.TemporaryDirectory() as temporary:
            participant_path = self.write(Path(temporary), "participantes.csv", PARTICIPANTS)
            exercise_path = self.write(Path(temporary), "ejercicios.csv", unsafe)
            participants = pilot.parse_participants(participant_path)
            with self.assertRaises(pilot.PilotDataError):
                pilot.parse_exercises(exercise_path, {p.code for p in participants})

    def test_rejects_unknown_participant(self) -> None:
        unknown = EXERCISES.replace("P-002", "P-999")
        with tempfile.TemporaryDirectory() as temporary:
            participant_path = self.write(Path(temporary), "participantes.csv", PARTICIPANTS)
            exercise_path = self.write(Path(temporary), "ejercicios.csv", unknown)
            participants = pilot.parse_participants(participant_path)
            with self.assertRaises(pilot.PilotDataError):
                pilot.parse_exercises(exercise_path, {p.code for p in participants})

    def test_writes_report_and_returns_zero(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            directory = Path(temporary)
            participant_path = self.write(directory, "participantes.csv", PARTICIPANTS)
            exercise_path = self.write(directory, "ejercicios.csv", EXERCISES)
            output = directory / "informe.md"
            code = pilot.main(
                [
                    "--participantes",
                    str(participant_path),
                    "--ejercicios",
                    str(exercise_path),
                    "--salida",
                    str(output),
                    "--sintetico",
                ]
            )
            self.assertEqual(code, 0)
            self.assertIn("Informe de piloto educativo", output.read_text(encoding="utf-8"))


if __name__ == "__main__":
    unittest.main()
