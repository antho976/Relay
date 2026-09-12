#!/usr/bin/env python3
"""Exercise the real shell helper with a fake ADB; no emulator or third-party packages needed."""
import json
import os
from pathlib import Path
import subprocess
import tempfile
import unittest

HELPER = Path(__file__).with_name("emulator_ready.sh").resolve()
FAKE_ADB = r'''#!/usr/bin/env python3
import json, os, sys, time
from pathlib import Path
state_path = Path(os.environ["ADB_STATE"])
state = json.loads(state_path.read_text())
args = sys.argv[1:]
state["commands"].append(args)
scenario = state["scenario"]
code = 0
if args == ["shell", "getprop", "sys.boot_completed"]:
    state["probes"] += 1
    if scenario == "hung":
        state_path.write_text(json.dumps(state))
        time.sleep(30)
    print("1\r")
elif args == ["shell", "pm", "path", "android"]:
    if scenario == "never" or (scenario == "delayed" and state["probes"] < 3):
        # Some shell failures return zero with an error string; transport success isn't readiness.
        print("cmd: Can't find service: package")
    else:
        print("package:/system/framework/framework-res.apk")
elif args == ["shell", "service", "check", "activity"]:
    print("Service activity: " + ("not found" if scenario == "no_activity" else "found"))
elif args[:1] == ["install"]:
    state["installs"] += 1
    if scenario == "bad_apk":
        print("Failure [INSTALL_PARSE_FAILED_NO_CERTIFICATES]")
        code = 1
    elif scenario == "install_race" and state["installs"] == 1 or scenario == "repeated_race":
        print("cmd: Can't find service: package")
        code = 1
    else:
        print("Success")
state_path.write_text(json.dumps(state))
sys.exit(code)
'''


class EmulatorReadyTest(unittest.TestCase):
    def run_scenario(self, scenario, command="install_release_apk 'release with spaces.apk'", timeout=10):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            adb = root / "adb"
            adb.write_text(FAKE_ADB)
            adb.chmod(0o755)
            state = root / "state.json"
            state.write_text(json.dumps(dict(scenario=scenario, probes=0, installs=0, commands=[])))
            env = dict(os.environ, PATH=f"{root}:{os.environ['PATH']}", ADB_STATE=str(state))
            # Skip only the cadence delay. The helper's real deadline and command timeout still run.
            result = subprocess.run(
                ["bash", "-c", 'set -euo pipefail; source "$1"; sleep() { :; }; ' + command, "test", str(HELPER)],
                env=env, text=True, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, timeout=timeout,
            )
            return result, json.loads(state.read_text())

    def test_ready_installs_once_after_two_successful_probes(self):
        result, state = self.run_scenario("ready")
        self.assertEqual(0, result.returncode, result.stdout)
        self.assertEqual(2, state["probes"])
        self.assertEqual(1, state["installs"])
        self.assertIn(["install", "-r", "-d", "release with spaces.apk"], state["commands"])

    def test_connected_but_package_service_not_ready_waits(self):
        result, state = self.run_scenario("delayed")
        self.assertEqual(0, result.returncode, result.stdout)
        self.assertEqual(4, state["probes"])
        self.assertEqual(1, state["installs"])

    def test_package_service_restart_between_probe_and_install_retries_once(self):
        result, state = self.run_scenario("install_race")
        self.assertEqual(0, result.returncode, result.stdout)
        self.assertEqual(2, state["installs"])
        self.assertEqual(4, state["probes"])

    def test_real_apk_error_fails_without_retry(self):
        result, state = self.run_scenario("bad_apk")
        self.assertNotEqual(0, result.returncode)
        self.assertEqual(1, state["installs"])
        self.assertIn("INSTALL_PARSE_FAILED", result.stdout)

    def test_repeated_service_failure_stays_red(self):
        result, state = self.run_scenario("repeated_race")
        self.assertNotEqual(0, result.returncode)
        self.assertEqual(2, state["installs"])

    def test_unready_or_hung_framework_expires_without_installing(self):
        for scenario in ("never", "no_activity", "hung"):
            with self.subTest(scenario=scenario):
                result, state = self.run_scenario(scenario, "wait_for_android_framework 2", timeout=6)
                self.assertNotEqual(0, result.returncode)
                self.assertEqual(0, state["installs"])
                self.assertIn("did not become ready", result.stdout)


if __name__ == "__main__":
    unittest.main()
