"""Bounded Rust subprocess protocol. All packages are imported from env/."""
import json
import sys

sys.path.insert(0, sys.argv[1])
import numpy as np
import onnxruntime as ort

request = json.load(sys.stdin)
options = ort.SessionOptions()
options.intra_op_num_threads = 1
options.inter_op_num_threads = 1
session = ort.InferenceSession(request["model"], sess_options=options,
                               providers=["CPUExecutionProvider"])

def schema(node):
    if node.type != "tensor(float)":
        raise ValueError("only f32 tensors supported")
    return {"name": node.name, "shape": node.shape, "dtype": "f32"}

if request["mode"] == "describe":
    response = {"inputs": [schema(n) for n in session.get_inputs()],
                "outputs": [schema(n) for n in session.get_outputs()]}
else:
    feeds = {t["name"]: np.asarray(t["data"], dtype=np.float32).reshape(t["shape"])
             for t in request["inputs"]}
    scores = session.run(None, feeds)[0].reshape(-1)
    labels = json.loads(session.get_modelmeta().custom_metadata_map["intent_labels"])
    if len(scores) != len(labels) or not np.isfinite(scores).all():
        raise ValueError("invalid ONNX intent output")
    response = [{"label": label, "score": float(score)} for label, score in zip(labels, scores)]
json.dump(response, sys.stdout, allow_nan=False)
