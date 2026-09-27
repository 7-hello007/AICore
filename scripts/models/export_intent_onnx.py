"""Export the checked-in deterministic JSON classifier; no training claims."""
import json
import pathlib
import sys
sys.path.insert(0, sys.argv[1])
import numpy as np
import onnx
from onnx import TensorProto, helper, numpy_helper

root = pathlib.Path(__file__).resolve().parents[2]
data = json.loads((root / "models/intent_classifier_v1.json").read_text())
graph = helper.make_graph([
    helper.make_node("MatMul", ["tokens", "weights"], ["logits"]),
    helper.make_node("Softmax", ["logits"], ["probabilities"], axis=1),
], "intent_classifier_v1",
    [helper.make_tensor_value_info("tokens", TensorProto.FLOAT, [1, data["feature_dim"]])],
    [helper.make_tensor_value_info("probabilities", TensorProto.FLOAT, [1, len(data["labels"])])],
    [numpy_helper.from_array(np.asarray(data["weights"], dtype=np.float32).T, "weights")])
model = helper.make_model(graph, opset_imports=[helper.make_opsetid("", 13)])
model.ir_version = 10
helper.set_model_props(model, {"intent_labels": json.dumps(data["labels"])})
onnx.checker.check_model(model)
onnx.save(model, root / "models/intent_classifier_v1.onnx")
print("Exported models/intent_classifier_v1.onnx")
