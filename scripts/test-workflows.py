# /// script
# requires-python = ">=3.11"
# dependencies = ["PyYAML==6.0.3"]
# ///
"""Regression tests for GitHub's PR-versus-release workflow contract."""

import json
from itertools import product
import unittest
from pathlib import Path

import yaml


ROOT = Path(__file__).resolve().parent.parent


def workflow(name):
    return yaml.safe_load((ROOT / ".github/workflows" / name).read_text())


def events(config):
    # PyYAML's YAML 1.1 loader reads GitHub's unquoted `on` key as True.
    return config.get("on", config.get(True))


def prerequisites(jobs, name):
    result = set()
    pending = [name]
    while pending:
        needs = jobs[pending.pop()].get("needs", [])
        for dependency in [needs] if isinstance(needs, str) else needs:
            if dependency not in result:
                result.add(dependency)
                pending.append(dependency)
    return result


def expanded_steps(job):
    if job.get("uses", "").startswith("./.github/workflows/"):
        for child in workflow(Path(job["uses"]).name)["jobs"].values():
            yield from expanded_steps(child)
    for step in job.get("steps", []):
        if step.get("uses", "").startswith("./.github/actions/"):
            action = yaml.safe_load((ROOT / step["uses"] / "action.yml").read_text())
            yield from expanded_steps(action["runs"])
        else:
            yield step


class WorkflowBoundaryTests(unittest.TestCase):
    def test_each_platform_has_an_independent_release_entry_point(self):
        for platform in ["linux", "macos-aarch64", "macos-x86_64", "windows-x86_64"]:
            config = workflow(f"release-{platform}.yml")
            trigger = events(config)
            self.assertNotIn("pull_request", trigger)
            self.assertEqual(trigger["push"]["tags"], [f"v*-rc.*-{platform}", f"v*-{platform}-preview.*"])
            self.assertIn("workflow_dispatch", trigger)
            self.assertEqual(list(config["jobs"]), ["release"])
            call = config["jobs"]["release"]
            self.assertNotIn("needs", call)
            self.assertEqual(call["uses"], "./.github/workflows/release-platform.yml")
            self.assertEqual(call["with"]["platform"], platform)
            self.assertEqual(call["secrets"], "inherit")

    def test_independent_publication_requires_its_own_successful_build(self):
        jobs = workflow("release-platform.yml")["jobs"]
        publish = jobs["publish_preview"]
        self.assertEqual(publish["environment"], "release-publishing")
        self.assertIn("needs.preflight.result == 'success'", publish["if"])
        self.assertIn("!cancelled()", publish["if"])
        for platform in ["linux", "macos", "windows"]:
            self.assertIn(f"needs.{platform}.result == 'success'", publish["if"])
            self.assertEqual(jobs[platform]["needs"], "preflight")
            self.assertNotIn("strategy", jobs[platform])
        self.assertNotIn("assemble", jobs)

    def test_preview_publication_expression_fails_closed_for_failed_or_skipped_selected_jobs(self):
        expression = workflow("release-platform.yml")["jobs"]["publish_preview"]["if"]
        for platform, selected in [("linux", "linux"), ("macos-aarch64", "macos"),
                                   ("macos-x86_64", "macos"), ("windows-x86_64", "windows")]:
            for preflight, build, cancelled, event in product(
                ["success", "failure", "skipped", "cancelled"],
                ["success", "failure", "skipped", "cancelled"],
                [False, True], ["push", "workflow_dispatch"]
            ):
                values = {
                    "cancelled()": repr(cancelled),
                    "github.event_name": repr(event), "inputs.platform": repr(platform),
                    "needs.preflight.result": repr(preflight),
                    **{f"needs.{name}.result": repr(build if name == selected else "skipped")
                       for name in ["macos", "windows", "linux"]},
                }
                actual = expression.removeprefix("${{").removesuffix("}}").strip()
                for key, value in values.items():
                    actual = actual.replace(key, value)
                actual = actual.replace("&&", "and").replace("||", "or").replace("!", "not ")
                actual = " ".join(actual.split())
                result = eval(actual, {"__builtins__": {}, "startsWith": str.startswith})
                expected = not cancelled and event == "push" and preflight == "success" and build == "success"
                self.assertEqual(result, expected, (platform, preflight, build, cancelled, event))

    def test_only_the_four_platforms_can_start_release_work(self):
        entry_points = {
            file.name for file in (ROOT / ".github/workflows").glob("release*.yml")
            if "push" in events(workflow(file.name))
        }
        self.assertEqual(entry_points, {
            "release-linux.yml", "release-macos-aarch64.yml",
            "release-macos-x86_64.yml", "release-windows-x86_64.yml",
        })
        shared = events(workflow("release-platform.yml"))
        self.assertEqual(set(shared), {"workflow_call"})
        self.assertEqual(set(shared["workflow_call"]["inputs"]), {"platform"})

    def test_preview_keeps_ci_and_publication_protection(self):
        jobs = workflow("release-platform.yml")["jobs"]
        preview = jobs["publish_preview"]
        self.assertEqual(preview["environment"], "release-publishing")
        self.assertIn("github.event_name == 'push'", preview["if"])
        downloads = [step for step in preview["steps"] if step.get("uses", "").startswith("actions/download-artifact@")]
        self.assertEqual(downloads[0]["with"]["name"], "mist-${{ inputs.platform == 'linux' && 'linux-x86_64' || inputs.platform }}")
        create = next(step["run"] for step in preview["steps"] if step.get("name") == "Create platform preview draft")
        for flag in ["--draft", "--prerelease", "--verify-tag", "--latest=false"]:
            self.assertIn(flag, create)
        self.assertNotIn("--clobber", create)
        self.assertIn("preflight", prerequisites(jobs, "publish_preview"))
        self.assertNotIn("environment", jobs["linux"])
        for name in ["macos", "windows"]:
            self.assertEqual(jobs[name]["environment"], "release-signing")

    def test_intel_build_is_required_and_uses_the_same_runtime_as_release(self):
        ci = workflow("ci.yml")
        check = ci["jobs"]["check"]
        self.assertIn("macos-15-intel", check["strategy"]["matrix"]["os"])
        runtime_action = "./.github/actions/intel-onnxruntime"
        ci_runtime = [step for step in check["steps"] if step.get("uses") == runtime_action]
        release_runtime = [
            step for step in workflow("release-platform.yml")["jobs"]["macos"]["steps"]
            if step.get("uses") == runtime_action
        ]
        self.assertEqual(len(ci_runtime), 1)
        self.assertEqual(ci_runtime[0]["if"], "matrix.os == 'macos-15-intel'")
        self.assertEqual(len(release_runtime), 1)
        self.assertEqual(release_runtime[0]["if"], "inputs.platform == 'macos-x86_64'")
        rules = json.loads((ROOT / ".github/rulesets/main-quality.json").read_text())
        required = next(
            rule["parameters"]["required_status_checks"]
            for rule in rules["rules"] if rule["type"] == "required_status_checks"
        )
        self.assertIn("check (macos-15-intel)", [check["context"] for check in required])

    def test_prs_run_ci_without_starting_a_release(self):
        ci = events(workflow("ci.yml"))
        for file in (ROOT / ".github/workflows").glob("release*.yml"):
            release = events(workflow(file.name))
            self.assertNotIn("pull_request", release)
            self.assertNotIn("pull_request_target", release)
            if "push" in release:
                self.assertNotIn("branches", release["push"])
                self.assertIn("workflow_dispatch", release)
        self.assertEqual(ci["pull_request"]["branches"], ["main"])
        self.assertEqual(ci["push"], {"branches": ["main"]})

    def test_linux_package_validation_remains_required_in_read_only_ci(self):
        ci = workflow("ci.yml")
        packages = [
            job for job in ci["jobs"].values()
            if job.get("name") == "Linux package validation"
        ]
        self.assertEqual(len(packages), 1)
        self.assertNotIn("if", packages[0])
        self.assertEqual(ci["permissions"], {"contents": "read"})
        for job in ci["jobs"].values():
            self.assertNotIn("environment", job)
        rules = json.loads((ROOT / ".github/rulesets/main-quality.json").read_text())
        required = next(
            rule["parameters"]["required_status_checks"]
            for rule in rules["rules"] if rule["type"] == "required_status_checks"
        )
        names = [check["context"] for check in required]
        self.assertIn("Linux package validation", names)
        self.assertNotIn("Linux release packages", names)

    def test_release_artifacts_require_exact_commit_ci_without_rerunning_tests(self):
        for file in ["release-platform.yml"]:
            jobs = workflow(file)["jobs"]
            self.assertNotIn("quality", jobs)
            gate = jobs["preflight"]
            self.assertEqual(gate["permissions"], {"contents": "read", "actions": "read"})
            self.assertNotIn("if", gate)
            checks = [step for step in gate["steps"] if "scripts/verify-release-ci.mjs" in step.get("run", "")]
            self.assertEqual(len(checks), 1)
            self.assertEqual(checks[0]["env"]["GH_TOKEN"], "${{ github.token }}")
            for name, job in jobs.items():
                for step in expanded_steps(job):
                    self.assertNotIn("continue-on-error", step)
                    self.assertNotIn("check-quality.sh", step.get("run", ""))
                    self.assertNotIn("cargo test", step.get("run", ""))
                if name != "preflight":
                    self.assertIn("preflight", prerequisites(jobs, name), name)
                    self.assertNotIn("always()", job.get("if", ""), name)
        quality_steps = list(expanded_steps(workflow("ci.yml")["jobs"]["quality"]))
        self.assertTrue(any("scripts/check-quality.sh" in step.get("run", "") for step in quality_steps))

    def test_every_release_checkout_is_pinned_to_the_event_commit(self):
        for file in ["release-platform.yml"]:
            for name, job in workflow(file)["jobs"].items():
                for step in job.get("steps", []):
                    if step.get("uses", "").startswith("actions/checkout@"):
                        self.assertEqual(step.get("with", {}).get("ref"), "${{ github.sha }}", name)


if __name__ == "__main__":
    unittest.main()
