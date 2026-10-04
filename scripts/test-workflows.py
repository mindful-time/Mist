# /// script
# requires-python = ">=3.11"
# dependencies = ["PyYAML==6.0.3"]
# ///
"""Regression tests for GitHub's PR-versus-release workflow contract."""

import json
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
    for step in job["steps"]:
        if step.get("uses", "").startswith("./.github/actions/"):
            action = yaml.safe_load((ROOT / step["uses"] / "action.yml").read_text())
            yield from expanded_steps(action["runs"])
        else:
            yield step


class WorkflowBoundaryTests(unittest.TestCase):
    def test_linux_preview_is_separate_and_keeps_ci_and_publication_protection(self):
        jobs = workflow("release.yml")["jobs"]
        preview = jobs["publish_linux_preview"]
        self.assertEqual(preview["environment"], "release-publishing")
        self.assertEqual(preview["needs"], ["preflight", "linux"])
        self.assertIn("needs.preflight.outputs.channel == 'linux-preview'", preview["if"])
        for name in ["macos", "windows", "assemble"]:
            self.assertIn("needs.preflight.outputs.channel != 'linux-preview'", jobs[name]["if"])
        downloads = [step for step in preview["steps"] if step.get("uses", "").startswith("actions/download-artifact@")]
        self.assertEqual(downloads[0]["with"]["name"], "mist-linux-x86_64")
        create = next(step["run"] for step in preview["steps"] if step.get("name") == "Create Linux preview draft")
        for flag in ["--draft", "--prerelease", "--verify-tag", "--latest=false"]:
            self.assertIn(flag, create)
        self.assertNotIn("--clobber", create)
        self.assertIn("preflight", prerequisites(jobs, "publish_linux_preview"))

    def test_intel_build_is_required_and_uses_the_same_runtime_as_release(self):
        ci = workflow("ci.yml")
        check = ci["jobs"]["check"]
        self.assertIn("macos-15-intel", check["strategy"]["matrix"]["os"])
        runtime_action = "./.github/actions/intel-onnxruntime"
        ci_runtime = [step for step in check["steps"] if step.get("uses") == runtime_action]
        release_runtime = [
            step for step in workflow("release.yml")["jobs"]["macos"]["steps"]
            if step.get("uses") == runtime_action
        ]
        self.assertEqual(len(ci_runtime), 1)
        self.assertEqual(ci_runtime[0]["if"], "matrix.os == 'macos-15-intel'")
        self.assertEqual(len(release_runtime), 1)
        self.assertEqual(release_runtime[0]["if"], "matrix.architecture == 'x86_64'")
        rules = json.loads((ROOT / ".github/rulesets/main-quality.json").read_text())
        required = next(
            rule["parameters"]["required_status_checks"]
            for rule in rules["rules"] if rule["type"] == "required_status_checks"
        )
        self.assertIn("check (macos-15-intel)", [check["context"] for check in required])

    def test_prs_run_ci_without_starting_a_release(self):
        ci = events(workflow("ci.yml"))
        release = events(workflow("release.yml"))
        self.assertNotIn("pull_request", release)
        self.assertEqual(release["push"]["tags"], ["v*"])
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
        jobs = workflow("release.yml")["jobs"]
        self.assertNotIn("quality", jobs)
        gate = jobs["preflight"]
        self.assertEqual(gate["permissions"], {"contents": "read", "actions": "read"})
        self.assertNotIn("if", gate)
        checks = [step for step in gate["steps"] if "scripts/verify-release-ci.mjs" in step.get("run", "")]
        self.assertEqual(len(checks), 1)
        self.assertEqual(checks[0]["env"]["GH_TOKEN"], "${{ github.token }}")
        for job in jobs.values():
            for step in list(expanded_steps(job)) + job["steps"]:
                self.assertNotIn("continue-on-error", step)
                self.assertNotIn("check-quality.sh", step.get("run", ""))
                self.assertNotIn("cargo test", step.get("run", ""))
        quality_steps = list(expanded_steps(workflow("ci.yml")["jobs"]["quality"]))
        self.assertTrue(any("scripts/check-quality.sh" in step.get("run", "") for step in quality_steps))
        for name in ["macos", "windows", "linux", "assemble", "publish"]:
            self.assertIn("preflight", prerequisites(jobs, name), name)
            self.assertNotIn("always()", jobs[name].get("if", ""), name)

    def test_every_release_checkout_is_pinned_to_the_event_commit(self):
        for name, job in workflow("release.yml")["jobs"].items():
            for step in job["steps"]:
                if step.get("uses", "").startswith("actions/checkout@"):
                    self.assertEqual(step.get("with", {}).get("ref"), "${{ github.sha }}", name)


if __name__ == "__main__":
    unittest.main()
