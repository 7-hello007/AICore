"""Generate an experimental QDQ policy candidate from the existing model.

Synthetic calibration and CPU equivalence sampling are not production accuracy
acceptance. The FP32 model remains the default on every backend.
"""
import pathlib,sys,json
root=pathlib.Path(__file__).resolve().parents[2]
if len(sys.argv) != 4:
 raise SystemExit("usage: python -I -S quantize_policy_npu.py <local ORT packages> <local onnx packages> <absolute env/ output>")
output=pathlib.Path(sys.argv[3]).resolve()
if root/'env' not in output.parents:
 raise SystemExit("candidate output must remain under project env/")
sys.path[:0]=[sys.argv[1],sys.argv[2]]
import numpy as np
import onnx
import onnxruntime as ort
from onnxruntime.quantization import quantize_static,CalibrationDataReader,QuantFormat,QuantType
model=root/'models/policy_classifier_v1.onnx'
output.parent.mkdir(parents=True,exist_ok=True)
class Samples(CalibrationDataReader):
 def __init__(self):
  rng=np.random.default_rng(20260928)
  self.rows=iter([{'features':a[np.newaxis,:]} for a in rng.uniform(0,1,size=(256,4)).astype(np.float32)])
 def get_next(self): return next(self.rows,None)
quantize_static(str(model),str(output),Samples(),quant_format=QuantFormat.QDQ,activation_type=QuantType.QUInt8,weight_type=QuantType.QInt8,op_types_to_quantize=['MatMul','Softmax'])
on=onnx.load(output);onnx.checker.check_model(on)
original=ort.InferenceSession(str(model),providers=['CPUExecutionProvider'])
quant=ort.InferenceSession(str(output),providers=['CPUExecutionProvider'])
rng=np.random.default_rng(17)
features=rng.uniform(0,1,size=(1000,4)).astype(np.float32)
maxerr=0; disagree=0
for x in features:
 a=original.run(None,{'features':x.reshape(1,4)})[0]
 b=quant.run(None,{'features':x.reshape(1,4)})[0]
 maxerr=max(maxerr,float(np.max(np.abs(a-b))))
 disagree+=int(np.argmax(a)!=np.argmax(b))
print(json.dumps({'model':str(output),'quantized_ops':[n.op_type for n in on.graph.node],'calibration':'deterministic synthetic coverage; not representative workload data','vectors_checked':len(features),'top_label_disagreement':disagree,'max_abs_score_error':maxerr,'metadata_labels':quant.get_modelmeta().custom_metadata_map.get('intent_labels')}))
