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
        if os.name == "nt":
            return script
        command = self.bin / name
        command.write_text("#!/usr/bin/env python3\n" + source)
        command.chmod(0o755)
        return command

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
            timeout=30,
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
    def test_git_bash_resolver_finds_install_root_from_supported_git_locations(self):
        resolver = ROOT / "scripts/resolve-git-bash.ps1"
        for git_location in ("cmd/git.exe", "bin/git.exe", "mingw64/bin/git.exe"):
            with self.subTest(git_location=git_location):
                install = self.root / git_location.replace("/", "-")
                git = install / git_location
                git.parent.mkdir(parents=True)
                git.touch()
                bash = install / "bin/bash.exe"
                bash.parent.mkdir(parents=True, exist_ok=True)
                bash.touch()
                (install / "cmd/git.exe").parent.mkdir(parents=True, exist_ok=True)
                (install / "cmd/git.exe").touch()
                result = subprocess.run(
                    [shutil.which("pwsh"), "-NoProfile", "-File", str(resolver), "-GitPath", str(git)],
                    cwd=ROOT,
                    text=True,
                    capture_output=True,
                    timeout=15,
                )
                self.assertEqual(result.returncode, 0, result.stderr)
                self.assertEqual(Path(result.stdout.strip()), bash)

    @unittest.skipUnless(shutil.which("pwsh"), "requires PowerShell Core")
    def test_distro_powershell_rejects_unpinned_image_before_docker(self):
        docker_called = self.root / "docker-called"
        if os.name == "nt":
            wrong_bash_called = self.root / "wrong-bash-called"
            (self.bin / "bash.cmd").write_text(
                '@echo off\r\necho called>>"%WRONG_BASH_CALLS%"\r\nexit /b 37\r\n'
            )
            (self.bin / "docker.cmd").write_text(
                '@echo off\r\necho called>>"%DOCKER_CALLS%"\r\nexit /b 0\r\n'
            )
        else:
            wrong_bash_called = None
            self.write_command(
                "docker",
                "import os\n"
                "from pathlib import Path\n"
                "Path(os.environ['DOCKER_CALLS']).write_text('called')\n",
            )
        image = "fedora image:latest"
        arguments = [
            "-Image", image,
            "-Crate", "tinyconnectors",
            "-Version", VERSION,
            "-Target", "x86_64-unknown-linux-gnu",
            "-HostKey", "fedora-43-x86_64",
            "-ExtraFilesJson", "[]",
            "-OutputDir", f"target/script-contract-{os.getpid()}",
        ]
        result = subprocess.run(
            [shutil.which("pwsh"), "-NoProfile", "-File", str(ROOT / "scripts/test-distro.ps1"), *arguments],
            cwd=ROOT,
            env=self.env(
                DOCKER_CALLS=str(docker_called),
                **({"WRONG_BASH_CALLS": str(wrong_bash_called)} if wrong_bash_called else {}),
            ),
            text=True,
            capture_output=True,
            timeout=30,
        )
        diagnostic = ""
        if result.returncode != 2:
            try:
                details = subprocess.run(
                    [
                        shutil.which("pwsh"), "-NoProfile", "-Command",
                        '$git=(Get-Command git).Source; '
                        '$bash=& ./scripts/resolve-git-bash.ps1 -GitPath $git; '
                        '"git source: $git`nresolved Git Bash: $bash"; '
                        '& $bash --version; "Git Bash launch exit: $LASTEXITCODE"; '
                        '"PATH bash: $((Get-Command bash -ErrorAction SilentlyContinue).Source)`nPATH: $env:PATH"',
                    ],
                    cwd=ROOT,
                    env=self.env(),
                    text=True,
                    capture_output=True,
                    timeout=15,
                )
                diagnostic = f"diagnostic stdout: {details.stdout}\ndiagnostic stderr: {details.stderr}"
            except subprocess.TimeoutExpired:
                diagnostic = "diagnostic timed out while probing Git Bash"
        self.assertEqual(
            result.returncode,
            2,
            f"stdout: {result.stdout}\nstderr: {result.stderr}\n{diagnostic}",
        )
        self.assertIn("unsupported module build image", result.stderr)
        self.assertIn(image, result.stderr, "PowerShell did not preserve the image argument")
        self.assertFalse(docker_called.exists(), "unpinned image reached Docker")
        if wrong_bash_called is not None:
            self.assertFalse(wrong_bash_called.exists(), "PowerShell selected the PATH bash launcher")

    @unittest.skipUnless(shutil.which("pwsh") and os.name != "nt", "requires POSIX PowerShell Core")
    def test_distro_powershell_delegates_arguments_and_exit_code_to_pinned_wrapper(self):
        runner_calls = self.root / "runner-args.json"
        self.write_command(
            "bash",
            "import json, os, sys\n"
            "open(os.environ['RUNNER_CALLS'], 'w', encoding='utf-8').write(json.dumps(sys.argv[1:]))\n"
            "raise SystemExit(37)\n",
        )
        wrapper = str(ROOT / "scripts/test-distro.sh")
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
        env = self.env(RUNNER_CALLS=str(runner_calls))
        result = subprocess.run(
            [shutil.which("pwsh"), "-NoProfile", "-File", str(ROOT / "scripts/test-distro.ps1"), *arguments],
            cwd=ROOT,
            env=env,
            text=True,
            capture_output=True,
            timeout=30,
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
