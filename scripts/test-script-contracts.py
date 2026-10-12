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

    def tearDown(self):
        self.fixture.cleanup()

    def write_command(self, name, source):
        script = self.bin / f"{name}.py"
        script.write_text(source)
        if os.name == "nt":
            # Git Bash and PowerShell do not consistently execute extensionless
            # Python shebangs. Use a native command shim so PATH interception is
            # the same on Windows as it is on Unix.
            path = self.bin / f"{name}.cmd"
            path.write_text(
                "@echo off\r\n"
                f'python "%MOCK_COMMAND_DIR%\\{name}.py" %*\r\n'
                "exit /b %errorlevel%\r\n"
            )
        else:
            path = self.bin / name
            path.write_text("#!/usr/bin/env python3\n" + source)
            path.chmod(0o755)
        return path

    def env(self, **values):
        return dict(
            os.environ,
            PATH=str(self.bin) + os.pathsep + os.environ["PATH"],
            MOCK_COMMAND_DIR=str(self.bin),
            **values,
        )

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
        docker_calls = self.root / "docker-args.json"
        self.write_command(
            "docker",
            "import json, os, sys\n"
            "open(os.environ['DOCKER_CALLS'], 'w', encoding='utf-8').write(json.dumps(sys.argv[1:]))\n"
            "raise SystemExit(int(os.environ.get('MOCK_DOCKER_EXIT', '0')))\n",
        )
        image = "fedora:43@sha256:a651ddf48ea28a06ed4e1e6519f51c9f47e7a5a138722ade87369b8fbb7e5b42"
        output_dir = f"target/script-contract-{os.getpid()}"
        arguments = [
            "-Image", image,
            "-Crate", "tinyconnectors",
            "-Version", VERSION,
            "-Target", "x86_64-unknown-linux-gnu",
            "-HostKey", "fedora-43-x86_64",
            "-ExtraFilesJson", "[]",
            "-OutputDir", output_dir,
        ]
        env = self.env(DOCKER_CALLS=str(docker_calls), MOCK_DOCKER_EXIT="37")
        result = subprocess.run(
            [shutil.which("pwsh"), "-NoProfile", "-File", str(ROOT / "scripts/test-distro.ps1"), *arguments],
            cwd=ROOT,
            env=env,
            text=True,
            capture_output=True,
        )
        self.assertEqual(result.returncode, 37, result.stderr)
        docker_args = json.loads(docker_calls.read_text())
        self.assertEqual(docker_args[:2], ["run", "--rm"])
        image_index = docker_args.index(image)
        self.assertEqual(docker_args[image_index + 1], "bash")
        self.assertIn("MODULE_CRATE=tinyconnectors", docker_args)
        self.assertIn(f"MODULE_VERSION={VERSION}", docker_args)
        self.assertIn("MODULE_TARGET=x86_64-unknown-linux-gnu", docker_args)
        self.assertIn("MODULE_HOST=fedora-43-x86_64", docker_args)

        docker_calls.unlink()
        arguments[arguments.index(image)] = "fedora:latest"
        rejected = subprocess.run(
            [shutil.which("pwsh"), "-NoProfile", "-File", str(ROOT / "scripts/test-distro.ps1"), *arguments],
            cwd=ROOT,
            env=env,
            text=True,
            capture_output=True,
        )
        self.assertNotEqual(rejected.returncode, 0, rejected.stdout)
        self.assertFalse(docker_calls.exists(), "unpinned image reached the Docker command")


if __name__ == "__main__":
    unittest.main()
