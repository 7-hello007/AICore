"""Explicit host ONNX regressions; requires existing project-local packages/EP."""
import json
import os
import pathlib
import subprocess
import sys
import unittest

ROOT = pathlib.Path(__file__).resolve().parents[2]
PACKAGES = pathlib.Path(os.environ["AICORE_ACCELERATOR_PACKAGES"]).resolve()
LIBRARY = os.environ["AICORE_ACCELERATOR_PROVIDER_LIBRARY"]


class NativeBridgeRegression(unittest.TestCase):
    def invoke(self, provider, library=None, model=None, features=None):
        request = {"mode": "infer", "model": str(model or ROOT / "models/policy_classifier_v1.onnx"),
                   "provider": provider, "device_id": 0, "provider_library": library,
                   "inputs": [{"name": "features", "shape": [1, 4], "data": features or [0.1, 0.9, 0.0, 0.1]}]}
        print("BEFORE target=" + provider + " ordinal=0 original=no child session proposed=isolated model inference rollback=release child session/EP and exit; no host configuration setters", flush=True)
        return subprocess.run([sys.executable, "-I", "-S", str(ROOT / "scripts/models/onnx_intent_runner.py"), str(PACKAGES)],
                              input=json.dumps(request), capture_output=True, text=True,
                              encoding="utf-8", errors="replace", timeout=30, cwd=ROOT)

    def assert_success(self, provider):
        result = self.invoke(provider)
        self.assertEqual(result.returncode, 0, result.stderr)
        output = json.loads(result.stdout)
        self.assertEqual(max(output, key=lambda value: value["score"])["label"], "boost_gpu")
        self.assertIn("provider_execution_verified=" + provider, result.stderr)
        self.assertAlmostEqual(sum(value["score"] for value in output), 1.0, places=5)
        print("RESTORATION PASS isolated " + provider + " child session exited", flush=True)

    def test_cpu_actual_nodes(self):
        self.assert_success("CPUExecutionProvider")

    def test_gpu_actual_nodes_no_cpu_fallback(self):
        self.assert_success("DmlExecutionProvider")

    def test_npu_candidate_actual_nodes(self):
        path = pathlib.Path(os.environ["AICORE_ACCELERATOR_MODEL_PATH"])
        result = self.invoke("VitisAIExecutionProvider", LIBRARY, path,
                             [0.1, 0.0, 0.9, 0.1])
        self.assertEqual(result.returncode, 0, result.stderr)
        labels = json.loads(result.stdout)
        self.assertEqual(max(labels, key=lambda value: value["score"])["label"], "boost_npu")
        self.assertIn("node_providers=VitisAIExecutionProvider", result.stderr)
        self.assertIn("provider_execution_verified=VitisAIExecutionProvider", result.stderr)
        self.assertIn("provider_registration_lifetime=owned_child_exit", result.stderr)
        print("RESTORATION PASS owned NPU child exited normally; no global settings changed", flush=True)

    def test_npu_unsupported_model_rejects_without_unload_crash(self):
        # Host-specific current baseline: tiny FP32 policy graph is not fully
        # assigned to this NPU. This proves fail-closed cleanup, not NPU inference.
        for _ in range(2):
            result = self.invoke("VitisAIExecutionProvider", LIBRARY)
            self.assertEqual(result.returncode, 1, result.stderr)
            self.assertIn("fallback to CPU EP has been explicitly disabled", result.stderr.replace("\x00", ""))
            self.assertIn("provider_registration_restored=VitisAIExecutionProvider", result.stderr)
            self.assertNotIn("provider_execution_verified=", result.stderr)
            print("RESTORATION PASS rejected NPU session unregisters and exits normally; original FP32 policy model NPU inference NOT VERIFIED", flush=True)


if __name__ == "__main__":
    unittest.main()

