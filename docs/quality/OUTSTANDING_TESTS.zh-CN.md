[英文](OUTSTANDING_TESTS.md) | [中文](OUTSTANDING_TESTS.zh-CN.md)

# 目标设计测试状态与待验收项

本文是当前未完成测试与验收的独立清单。历史[验证报告](DESTINATION_VERIFICATION_REPORT.zh-CN.md)保留各次运行的原始证据，较早章节不覆盖本文。**PASS** 只对表内写明的范围成立；**FAIL** 表示实际观察到失败或不兼容；**SKIPPED** 表示有意不运行；**NOT VERIFIED** 表示缺少必要的验收证据。

下表的项目级证据来自本轮文档收尾之前的桌面验证，日志位于 `env/validation/native-1790517148896/`。本轮不做原生控制，也不修改宿主机配置。

## 已完成的检查

| 检查 | 状态 | 证据与准确范围 |
|---|---|---|
| Windows 工作区 `cargo check --workspace --all-targets --locked --offline` | PASS | 退出码 0；`check-closure.log`；Rust 1.75.0。 |
| Windows 工作区 `cargo clippy --workspace --all-targets --all-features --locked --offline -- -D warnings` | PASS | 退出码 0；`clippy-closure.log`。 |
| Windows 工作区 `cargo test --workspace --all-targets --locked --offline -- --nocapture` | PASS | 退出码 0；147 个测试目标汇总，730 通过、0 失败、0 忽略、0 过滤；`tests-closure.log`。这是上一轮已获授权的原生测试，本轮未重跑。 |
| `cargo fmt --all -- --check` | PASS | 退出码 0；`fmt-closure.log`。 |
| Linux 目标的工作区编译与 Clippy | PASS | 退出码均为 0；`linux-check-closure.log`、`linux-clippy-closure.log`；仅证明交叉编译。 |
| 模型守护进程单次运行 | PASS | 一个模型控制周期；`model-closure.log`；不代表硬件执行。 |
| Windows 模拟平台模式与命名管道冒烟测试 | PASS | 10 个控制周期、5 次内存执行成功、0 失败；`simulator-closure.log`；仅模拟执行。 |
| 本机 Ollama `llama3.2:3b` 经模拟守护进程 IPC 执行集成意图测试 | PASS | 返回类型化意图；`ollama-closure.log`；不代表意图准确率认证。 |
| 原生 DirectML GPU 推理与设备队列探针 | PASS | AMD Radeon 860M 上的设备放置；`native-gpu-2000-safe.log`；是单独的直接推理，不是守护进程自动 GPU 路由。 |
| 实验性 QDQ NPU 推理与队列探针 | PASS | VitisAI 节点分析证据；`npu-qdq-final.log`、`npu-closure.log`；仅针对候选模型。 |
| 受控子进程优先级、亲和性及电源方案恢复 | PASS | 操作前后读回并恢复原值；`native-tests.log`、`native-power.log`；上一轮已获授权的原生测试。 |
| 仅允许所有者的管道拒绝匿名身份并接受所有者 | PASS | S-1-5-7 返回 `ACCESS_DENIED (5)`，所有者连接成功；不等于第二登录用户测试。 |
| 有限时长的原生控制周期调用样本 | PASS | 2,000 次调用、823 次受控进程操作结果；第 99 百分位耗时 1.2307 毫秒、最大耗时 1.8938 毫秒，20 毫秒调用预算内 0 次超时；`native-gpu-2000-safe.log`；不代表硬实时保证。 |
| 隔离的实际观测效果与个人记忆探针 | PASS | 12 对短时受控 CPU 基线与干预样本，以及同意、保留和撤销检查；`native-effect-memory.log`；不代表典型应用收益。 |
| 静态审计、Shell/PowerShell/Python 语法与 Git 空白检查 | PASS | `static-closure.log`、`git-diff-check.log`。 |

## 未完成测试及原因

| 验收检查 | 状态 | 原因与下一项证据 |
|---|---|---|
| 生产默认 FP32 策略模型在 NPU 上放置 | FAIL | 已安装 VitisAI 执行提供者将默认计算图全部分配到 CPU。关闭 CPU 回退后按设计以退出码 1 拒绝执行（`npu-inference-final.log`）。实验性 QDQ 模型在 1,000 个合成向量上有 16 个最高标签差异，尚不能替代生产默认模型。 |
| 原生守护进程自动 GPU/NPU 模型路由 | NOT VERIFIED | 本机能发现设备身份，但 Windows 适配器无法读出该机的 GPU/NPU 利用率；修正后的特征链路拒绝推理，不把缺失值伪装为零。修复后探针的加速器绑定次数为 0，需要有证据的特征来源或验证过的新契约。 |
| 持续负载下的端到端硬截止期限 | NOT VERIFIED | 有限控制周期样本没有覆盖完整守护进程状态持久化、IPC 耗时、长时间竞争及调度抖动。需在约定工作负载下测量全路径分布与超时。 |
| 第二位实际登录用户的 Windows 命名管道拒绝 | NOT VERIFIED | 匿名身份拒绝和所有者访问已通过，但没有从另一登录账户运行客户端。需使用不同登录账户执行已有客户端，无需在聊天中提供密码。 |
| 生产环境的类型化 GPU/NPU/电源插件控制 | NOT VERIFIED | 模拟提供者中有默认拒绝的能力绑定，但该厂商尚无接入生产守护进程的独立设置器与读回契约。仍需原生插件进程、权限拒绝与回滚证据。 |
| 代表性应用效果、能耗与长期个人预测 | NOT VERIFIED | 12 对短时受控 CPU 样本和一分钟只读前台样本，不能构成已获同意的代表性应用任务语料或长期预测与收益研究。需注册任务、对照组及能耗与吞吐实测。 |
| 跨厂商与 Linux 原生运行矩阵 | NOT VERIFIED | Windows 本机与 Linux 交叉编译不能认证其他厂商设备或 Linux 运行时。需在目标机器进行按能力门控的原生测试。 |
| 服务生命周期与安装包认证 | SKIPPED | 本副本明确禁止运行安装包；未安装服务或系统包。需要另行授权的可丢弃环境。 |
| 泛化自主工具代理协作 | SKIPPED | 工具权限、模式、取消和回滚的验收尚未定义；现有类型化意图协调范围较窄。 |
| 内核集成实现 | SKIPPED | 必要性分析没有发现必须使用内核组件的证据；在出现可测量需求与设计之前，不存在相应实现测试。 |

本轮不执行原生控制测试。前一次完整工作区测试保留为历史证据，不能写成本轮新执行结果。
