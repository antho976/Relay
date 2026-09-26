"""Run the audit reproductions against compiled production classes and pinned libraries.

First run the Gradle gate documented in ../../RELEASE_AUDIT_2026-09-12.md.
Uses existing Gradle caches only; creates compiler output in a temporary directory.
These probes demonstrate current defects. They are evidence, not passing regressions.
"""
from pathlib import Path
import os
import subprocess
import tempfile
import zipfile

HERE = Path(__file__).resolve().parent
REPO = HERE.parents[2]
CACHE = Path(os.environ.get("GRADLE_USER_HOME", Path.home() / ".gradle")) / "caches/modules-2/files-2.1"


def artifact(group, name, version, suffix):
    matches = sorted((CACHE / group / name / version).glob(f"*/{name}-{version}.{suffix}"))
    if not matches:
        raise SystemExit(f"Missing cached {group}:{name}:{version}; run the documented Gradle gate.")
    return matches[0]


def run():
    app_classes = REPO / "forge-android/app/build/tmp/kotlin-classes/debug"
    if not app_classes.is_dir():
        raise SystemExit("Compiled app classes missing; run the documented Gradle gate first.")
    stdlib = artifact("org.jetbrains.kotlin", "kotlin-stdlib", "2.2.10", "jar")
    work = artifact("androidx.work", "work-runtime", "2.10.1", "aar")
    with tempfile.TemporaryDirectory(prefix="avex-audit-") as directory:
        output = Path(directory)
        work_classes = output / "work-runtime.jar"
        with zipfile.ZipFile(work) as archive:
            work_classes.write_bytes(archive.read("classes.jar"))
        for name, dependency in [("AvexDomainProbe", app_classes), ("AvexWorkScheduleProbe", work_classes)]:
            classpath = os.pathsep.join(map(str, [output, dependency, stdlib]))
            subprocess.run(["javac", "-cp", classpath, "-d", str(output), str(HERE / f"{name}.java")], check=True)
            subprocess.run(["java", "-cp", classpath, name], check=True)


if __name__ == "__main__":
    run()
