"""Offline contract tests for the module's platform entrypoints."""
import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import unittest

ROOT = Path(__file__).resolve().parent.parent
TARGET = "x86_64-pc-windows-msvc"
VERSION = "0.14.2"


class ScriptContractTests(unittest.TestCase):
    def setUp(self):
        (ROOT / "target").mkdir(exist_ok=True)
        self.fixture = tempfile.TemporaryDirectory(dir=ROOT / "target")
        self.root = Path(self.fixture.name)
        self.bin = self.root / "bin"
        self.bin.mkdir()
        self.bash_env = self.root / "bash-env.sh"
        self.bash_env.write_text(
            'rustc() { python3 "$MOCK_COMMAND_DIR/rustc.py" "$@"; }\n'
            'cargo() { python3 "$MOCK_COMMAND_DIR/cargo.py" "$@"; }\n'
            'pwsh() { python3 "$MOCK_COMMAND_DIR/pwsh.py" "$@"; }\n'
            'docker() { python3 "$MOCK_COMMAND_DIR/docker.py" "$@"; }\n'
        )

    def tearDown(self):
        self.fixture.cleanup()

    def write_command(self, name, source):
        script = self.bin / f"{name}.py"
        script.write_text(source)
        return script

    def env(self, **values):
        environment = dict(
            os.environ,
            PATH=str(self.bin) + os.pathsep + os.environ["PATH"],
            **values,
        )
        if os.name != "nt":
            environment["BASH_ENV"] = self.bash_env.relative_to(ROOT).as_posix()
            environment["MOCK_COMMAND_DIR"] = self.bin.relative_to(ROOT).as_posix()
        return environment

    @unittest.skipIf(os.name == "nt", "Windows shell dispatch is exercised by module-e2e")
    def test_windows_shell_entrypoint_delegates_to_test_e2e_ps1_with_metadata(self):
        calls = self.root / "pwsh-calls.jsonl"
        self.write_command("rustc", "print('host: x86_64-pc-windows-msvc')\n")
        self.write_command(
            "cargo",
            "import json\nprint(json.dumps({'packages':[{'name':'tinyconnectors','version':'0.14.2'}]}))\n",
        )
        self.write_command(
            "pwsh",
            "import json, os, sys\n"
            "with open(os.environ['CALLS'], 'a', encoding='utf-8') as f: f.write(json.dumps(sys.argv[1:])+'\\n')\n"
            "raise SystemExit(int(os.environ.get('MOCK_PWSH_EXIT', '0')))\n",
        )
        env = self.env(RUNNER_OS="Windows", CALLS=str(calls), MOCK_PWSH_EXIT="29")
        result = subprocess.run(
            ["bash", str(ROOT / "scripts/test-e2e.sh")],
            cwd=ROOT,
            env=env,
            text=True,
            capture_output=True,
        )
        self.assertEqual(result.returncode, 29, result.stderr)
        calls_made = [json.loads(line) for line in calls.read_text().splitlines()]
        self.assertEqual(
            calls_made,
            [[
                "-NoProfile", "-File", "scripts/test-e2e.ps1",
                "-Target", TARGET, "-Version", VERSION,
            ]],
        )

    def test_distro_powershell_entrypoint_exists(self):
        self.assertTrue((ROOT / "scripts/test-distro.ps1").is_file())

    @unittest.skipUnless(shutil.which("pwsh"), "requires PowerShell Core")
    def test_distro_powershell_delegates_arguments_and_exit_code_to_pinned_wrapper(self):
        runner_calls = self.root / "runner-args.json"
        bash_mock = self.root / "bash-mock.ps1"
        bash_mock.write_text(
            "function bash {\n"
            "  [System.IO.File]::WriteAllText($env:RUNNER_CALLS, "
            "(ConvertTo-Json -InputObject @($args) -Compress))\n"
            "  $global:LASTEXITCODE = 37\n"
            "}\n"
            ". $env:TEST_DISTRO_SCRIPT -Image $env:IMAGE -Crate tinyconnectors "
            "-Version $env:MODULE_VERSION -Target x86_64-unknown-linux-gnu "
            "-HostKey fedora-43-x86_64 -ExtraFilesJson '[]' -OutputDir $env:OUTPUT_DIR\n"
        )
        wrapper = str(ROOT / "scripts/test-distro.sh")
        image = "fedora:43@sha256:a651ddf48ea28a06ed4e1e6519f51c9f47e7a5a138722ade87369b8fbb7e5b42"
        output_dir = f"target/script-contract-{os.getpid()}"
        env = self.env(
            RUNNER_CALLS=str(runner_calls),
            TEST_DISTRO_SCRIPT=str(ROOT / "scripts/test-distro.ps1"),
            IMAGE=image,
            MODULE_VERSION=VERSION,
            OUTPUT_DIR=output_dir,
        )
        result = subprocess.run(
            [shutil.which("pwsh"), "-NoProfile", "-File", str(bash_mock)],
            cwd=ROOT,
            env=env,
            text=True,
            capture_output=True,
        )
        self.assertEqual(result.returncode, 37, result.stderr)
        self.assertEqual(
            json.loads(runner_calls.read_text()),
            [
                wrapper, image,
                "--crate", "tinyconnectors",
                "--version", VERSION,
                "--target", "x86_64-unknown-linux-gnu",
                "--host", "fedora-43-x86_64",
                "--extra-files-json", "[]",
                "--output-dir", output_dir,
            ],
        )


if __name__ == "__main__":
    unittest.main()
