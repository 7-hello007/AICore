"""Bounded Rust subprocess protocol; imports and temporary output use env/."""
import gc
import hashlib
import json
import os
import pathlib
import sys
import tempfile
import traceback
sys.path.insert(0, sys.argv[1])
import numpy as np
import onnxruntime as ort

# Plugin EP registrations with live sessions stay in this one-request child.
PROCESS_SCOPED_PROVIDER = False


def execute(request):
    global PROCESS_SCOPED_PROVIDER
    provider = request.get("provider", "CPUExecutionProvider")
    library = request.get("provider_library")
    registered = False
    dll_directory = None
    session = None
    options = None
    devices = None
    failure = None
    original_cwd = os.getcwd()
    root = pathlib.Path(sys.argv[1]).resolve().parent
    try:
        if library:
            path = pathlib.Path(library)
            if not path.is_absolute() or not path.is_file():
                raise ValueError("provider library must be an existing absolute path")
            if os.name == "nt":
                dll_directory = os.add_dll_directory(str(path.parent))
            ort.register_execution_provider_library(provider, str(path))
            registered = True
        if request["mode"] == "probe":
            return {"providers": ort.get_available_providers(),
                    "devices": [{"provider": d.ep_name, "id": d.device.device_id,
                                 "type": str(d.device.type)} for d in ort.get_ep_devices()]}
        options = ort.SessionOptions()
        options.intra_op_num_threads = 1
        options.inter_op_num_threads = 1
        options.enable_mem_pattern = False
        options.execution_mode = ort.ExecutionMode.ORT_SEQUENTIAL
        if provider != "CPUExecutionProvider" and not request.get("diagnose_partition", False):
            options.add_session_config_entry("session.disable_cpu_ep_fallback", "1")
        kwargs = {}
        if provider == "VitisAIExecutionProvider":
            devices = [d for d in ort.get_ep_devices() if d.ep_name == provider
                       and d.device.type == ort.OrtHardwareDeviceType.NPU]
            ordinal = request.get("device_id")
            if ordinal is None or ordinal < 0 or ordinal >= len(devices):
                raise RuntimeError("explicit NPU ordinal unavailable")
            cache = root / "npu-cache" / hashlib.sha256(pathlib.Path(request["model"]).read_bytes()).hexdigest()
            cache.mkdir(parents=True, exist_ok=True)
            options.add_provider_for_devices([devices[ordinal]],
                {"cache_dir": str(cache), "enable_cache_file_io_in_mem": "0"})
        else:
            if provider not in ort.get_available_providers():
                raise RuntimeError("explicit provider unavailable: " + provider)
            provider_options = {"device_id": str(request["device_id"])} if provider == "DmlExecutionProvider" else {}
            kwargs["providers"] = [(provider, provider_options)]
        with tempfile.TemporaryDirectory(dir=root, ignore_cleanup_errors=True) as scratch:
            os.chdir(scratch)
            os.environ["TEMP"] = scratch
            os.environ["TMP"] = scratch
            if request["mode"] != "describe":
                options.enable_profiling = True
                options.profile_file_prefix = str(pathlib.Path(scratch) / "inference")
            session = ort.InferenceSession(request["model"], sess_options=options, **kwargs)
            if provider == "VitisAIExecutionProvider":
                PROCESS_SCOPED_PROVIDER = True
            session.disable_fallback()
            if provider not in session.get_providers():
                raise RuntimeError("requested provider was not activated")
            if request["mode"] == "describe":
                def schema(node):
                    if node.type != "tensor(float)":
                        raise ValueError("only f32 tensors supported")
                    return {"name": node.name, "shape": node.shape, "dtype": "f32"}
                response = {"inputs": [schema(n) for n in session.get_inputs()],
                            "outputs": [schema(n) for n in session.get_outputs()]}
            else:
                feeds = {t["name"]: np.asarray(t["data"], dtype=np.float32).reshape(t["shape"])
                         for t in request["inputs"]}
                scores = session.run(None, feeds)[0].reshape(-1)
                events = json.loads(pathlib.Path(session.end_profiling()).read_text())
                executed = {event.get("args", {}).get("provider") for event in events if event.get("cat") == "Node"}
                executed.discard(None)
                print("node_providers=" + ",".join(sorted(executed)), file=sys.stderr)
                if executed != {provider}:
                    raise RuntimeError("execution provider proof differs: " + str(executed))
                print("provider_execution_verified=" + provider, file=sys.stderr)
                labels = json.loads(session.get_modelmeta().custom_metadata_map["intent_labels"])
                if len(scores) != len(labels) or not np.isfinite(scores).all():
                    raise ValueError("invalid ONNX intent output")
                response = [{"label": label, "score": float(score)} for label, score in zip(labels, scores)]
            session = None
            os.chdir(original_cwd)
            return response
    except Exception as error:
        failure = str(error)
        # Release partial native sessions held by the original traceback before
        # unloading an EP DLL. Otherwise their destructors can call freed code.
        traceback.clear_frames(error.__traceback__)
        error.__traceback__ = None
    finally:
        session = None
        options = None
        devices = None
        gc.collect()
        os.chdir(original_cwd)
        if registered:
            if PROCESS_SCOPED_PROVIDER:
                print("provider_registration_lifetime=owned_child_exit", file=sys.stderr)
            else:
                ort.unregister_execution_provider_library(provider)
                print("provider_registration_restored=" + provider, file=sys.stderr)
        if dll_directory is not None:
            dll_directory.close()
    if failure is not None:
        raise RuntimeError(failure)


if __name__ == "__main__":
    try:
        json.dump(execute(json.load(sys.stdin)), sys.stdout, allow_nan=False)
    except BaseException:
        traceback.print_exc()
        sys.stdout.flush()
        sys.stderr.flush()
        if PROCESS_SCOPED_PROVIDER:
            os._exit(1)
        sys.exit(1)
    sys.stdout.flush()
    sys.stderr.flush()
    if PROCESS_SCOPED_PROVIDER:
        os._exit(0)
