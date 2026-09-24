# Plugin Lifecycle

`Discover → Manifest Validate → Trust Validate → Capability Bind → Start → Health → Stop → Update/Rollback → Remove`

生命周期必须和业务执行解耦；Plugin 崩溃不能导致 Core 进程崩溃。

## 当前代码状态

`PluginActivationManager` 仅在操作员通过 `--plugin-policy` 指定的 pin 中找到
唯一匹配项时尝试激活。pin 包含绝对 manifest/artifact 路径、两份 SHA-256、
trust level、可选 Ed25519 签名与公钥、允许接口/权限和进程预算。激活前重读并
核对 manifest 与 artifact，随后把已验证字节放入私有暂存目录；进程完成
Ping/Pong 后才进入 Active。健康轮询发现崩溃时解绑并标记 Failed，stop/Drop
会终止受管进程。Model Mode 和 simulated-platform Mode 禁止外部插件激活。

目前绑定 `extension.execute`、有类型的只读 `telemetry.snapshot_v1`，以及
限定进程优先级的 `process.priority_v1`。遥测接口
要求 Telemetry kind、`telemetry.read` 权限和操作员 pin，响应通过
`ManagedPluginTelemetryProvider` 实现遥测 trait；插件不可用时返回错误。
失败进程可凭同一 pin 显式重新激活。`replace_active` 先验证、暂存、启动并
探活新 artifact，先在注册表锁内验证替换条件，再停止旧进程并提交新 manifest，
最后切换句柄；候选验证或旧进程停止失败时保留旧绑定。操作员可传旧版本 pin
执行回滚。优先级接口要求 Adapter kind、`process.control` 与操作员控制 pin；
daemon 只选唯一活跃插件，ACK 后由平台适配器独立读回。插件失败不回落到
原生写入；affinity、GPU/NPU、电源插件控制不在此接口内。没有自动更新或
自动重启策略。
daemon 的 Platform Mode 只有恰好一个已激活 typed 遥测插件时才选择它；
读取失败会退回平台只读遥测。Model Mode 和 simulated-platform Mode 禁止启动外部插件，因此该接线
只有 mock/代码证据，尚无真实控制插件进程或 Windows 隔离证据。
