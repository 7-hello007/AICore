[英文](INTENT_BACKENDS.md) | [中文](INTENT_BACKENDS.zh-CN.md)
# 本地意图提供者

核心层消费 `IIntentAgent` 和经过校验的 `IntentProposal`；后端选择、词元词表与模型放置均在核心层之外。意图输出只是现有策略引擎与校验器的语义输入，绝不授予硬件控制权限。守护进程与冒烟示例共用 `configured_intent_agent`。

| `AICORE_INTENT_BACKEND` | 实际实现 | 依赖 |
|---|---|---|
| `ollama`（默认） | 通过本机回环地址调用 Ollama 推理 | 已运行的 Ollama；`AICORE_OLLAMA_MODEL` 默认值为 `llama3.2:3b` |
| `json` | `PureRustRuntime` 的加权词元分类 | `AICORE_INTENT_MODEL_PATH`，默认值为 `models/intent_classifier_v1.json` |
| `onnx` | 通过 `PythonOnnxSessionFactory` 启动显式 ONNX 运行时子进程，默认使用 CPU | 显式配置 `AICORE_ONNX_PYTHON`；项目内 `env/onnx-packages` 软件包 |
| `ensemble` | 并发运行有界的 JSON 与 Ollama 提案，并以加权共识合并 | 上述两个提供者；意见分歧或提案过迟时失败 |
| `disabled` | 不启用意图提供者 | API 返回 `NotSupported` |

移动二进制文件时，应通过 `AICORE_PROJECT_ROOT` 指定部署项目根目录的绝对路径；开发环境可回退至构建检出目录。`AICORE_ONNX_MODEL_PATH` 可独立选择 ONNX 模型；默认路径为 JSON 清单路径改用 `.onnx` 扩展名。词表长度、实际输入与输出张量形状以及 F32 类型必须与清单一致。标签是类型化的工作负载名称。提供者拒绝未知输入、无效概率、过期提案及低于当前校验阈值的置信度。

附带的 JSON 模型是**手工编写的分类基线**，不代表训练过的语言模型或经过校准的准确率。`scripts/models/export_intent_onnx.py` 把相同权重导出为 MatMul 加 Softmax；仓库中的 ONNX 文件即该导出结果。更换模型或词表无需修改核心层。任意生成式 ONNX 语言模型需要不同的词元化与生成适配器；该分类器不宣称支持它们。

ONNX 桥接层使用现有 Python 解释器及 `-I -S` 参数，只加载项目内的软件包，默认采用 `CPUExecutionProvider`，并校验实际模型模式、持续读取标准输出与错误输出、限制协议字节数、终止超时子进程。它不会下载依赖，也不会回退到模拟提供者。`AICORE_ONNX_PROVIDER` 可选择 CPU、`DmlExecutionProvider` 或 `VitisAIExecutionProvider`；`AICORE_ONNX_DEVICE_ID` 显式给出设备序号。DirectML 需要 DXGI 序号。NPU 选择会列举真实的执行提供者设备并要求有效序号，不会只凭厂商名称猜测。`AICORE_ONNX_PACKAGES` 指定隔离的软件包目录；可选的 `AICORE_ONNX_PROVIDER_LIBRARY` 指向已安装执行提供者库的绝对路径。库注册仅作用于子进程，正常成功或失败时会显式注销；子进程被终止后也会释放自身状态。不会调用应用商店或提供者的自动安装、下载接口。加速器会话禁用 CPU 回退，并要求节点确实在请求的执行提供者上运行的分析证据。临时输出与 NPU 模型缓存保存在项目 `env/` 下。每个子进程创建一次会话，不提供常驻服务。

守护进程的策略模型设备队列使用 `AICORE_ACCELERATOR_PROVIDER`、`AICORE_ACCELERATOR_DEVICE_ID`、`AICORE_ACCELERATOR_PACKAGES`、可选的 `AICORE_ACCELERATOR_PROVIDER_LIBRARY` 与绝对路径 `AICORE_ACCELERATOR_MODEL_PATH`，并复用 `AICORE_ONNX_PYTHON`。仓库中的 `policy_classifier_v1.onnx` 导出既有 JSON 权重，消费者特征维度为 **4**，输出维度为 **5**。排队的会话在慢侧执行有界且受能力约束的模型分派。回退只能作用于会话，硬件设置器不属于慢侧准备。Windows 组合层发现所选 DXGI 设备；NPU 绑定目前要求唯一明确的即插即用 NPU 及执行提供者序号 0。多个 NPU 的身份映射无法确定时默认拒绝。队列容量为 64，后端并发数为 1。五秒的子进程限制和过期检查拒绝过迟输出。只有会话实际创建成功后，才添加所选提供者的能力。配置失败会记录警告并保留平台回退，不宣称完成加速器执行。

在本机，DirectML 推理通过了节点提供者证据检查。AMD WinML NPU 执行提供者库成功注册并发现 NPU。默认 FP32 策略模型在该提供者下只分配到 CPU；禁用 CPU 回退时会按设计拒绝执行。`scripts/models/quantize_policy_npu.py` 可在 `env/` 下生成**实验性** QDQ 候选模型；显式指定其绝对路径后，所有已分析节点在 NPU 上执行，Rust 队列也返回了真实推理结果。与 FP32 比较的 1,000 个确定性合成样本中，最大分数差为 0.00354925，最高分标签有 16 次不同，因此该候选模型尚不能作为生产等效替代品。默认模型仍为 FP32。NPU 执行提供者每个子进程只处理一个请求；会话创建后，注册状态随子进程退出，以避免厂商库卸载崩溃；创建失败时正常注销。实验候选模型的 NPU 推理为 **PASS**，生产策略模型的 NPU 验收仍是 **NOT VERIFIED**。测试未修改时钟、电源或全局设备设置。本机 Windows 适配器能识别 GPU/NPU 身份，但读不到其利用率；因此 `AdaptiveLoop` 拒绝设备模型推理，不将缺失值当作零。直接队列探针成功不等于生产守护进程自动路由成功。原生证据见当前目标设计验证报告。

`IntentAgentCoordinator` 最多并发分派四个有界提供者。只有符合资格、及时且数值有限的提案可以投票；平局会失败，共识到期时间采用参与者中最早的到期时间。守护进程的默认集成不使用云端提供者。该组件协调分类提案，不能执行策略或启动独立的自主工具代理。

验证命令（使用现有配置的 Rust 1.75 环境）：

```powershell
$env:AICORE_INTENT_BACKEND='json'
cargo run -p aicore-ai --example configured_intent --locked --offline
$env:AICORE_INTENT_BACKEND='onnx'
$env:AICORE_ONNX_PYTHON='C:\Users\12282\anaconda3\python.exe'
cargo run -p aicore-ai --example configured_intent --locked --offline
./scripts/verify/simulated_platform_ollama_intent.ps1 -Backend ensemble
```

上述解释器路径是该验证主机已有的路径，不是可移植的默认值。当次运行下载的依赖仅存放在 `env/onnx-packages` 和 `env/pip-cache`；证据见目标设计验证报告。