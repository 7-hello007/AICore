[英文](PLUGIN_LIFECYCLE.md) | [中文](PLUGIN_LIFECYCLE.zh-CN.md)

# 插件生命周期

`Discover → Manifest Validate → Trust Validate → Capability Bind → Start → Health → Stop → Update/Rollback → Remove`

生命周期必须和业务执行解耦；插件崩溃不能导致核心进程崩溃。

## 当前代码状态

`PluginActivationManager` 仅在操作员通过 `--plugin-policy` 指定的固定策略中找到
唯一匹配项时尝试激活。固定策略包含清单和制品的绝对路径、两份 SHA-256、
信任等级、可选的 Ed25519 签名与公钥、允许的接口与权限及进程预算。激活前重读并
核对清单与制品，随后把已验证字节放入私有暂存目录；进程完成
探活握手后才进入运行状态。健康轮询发现崩溃时解绑并标记失败；停止或对象销毁
会终止受管进程。模型模式和模拟平台模式禁止激活外部插件。

目前绑定 `extension.execute`、类型化的只读 `telemetry.snapshot_v1`，以及
限定进程优先级的 `process.priority_v1`。遥测接口
要求遥测插件类型、`telemetry.read` 权限和操作员固定策略，响应通过
`ManagedPluginTelemetryProvider` 实现遥测接口；插件不可用时返回错误。
失败进程可凭同一固定策略显式重新激活。`replace_active` 先验证、暂存、启动并
探活新制品，在注册表锁内验证替换条件，再停止旧进程并提交新清单，
最后切换句柄；候选验证或旧进程停止失败时保留旧绑定。操作员可传旧版本固定策略
执行回滚。优先级接口要求适配器插件类型、`process.control` 权限及操作员控制固定策略；
守护进程只选择唯一活跃插件，收到确认后由平台适配器独立读回。插件失败不回落到
原生写入；亲和性、GPU/NPU 和电源插件控制不在此接口内。没有自动更新或
自动重启策略。
守护进程的平台模式只有恰好一个已激活的类型化遥测插件时才选择它；
读取失败会退回平台只读遥测。模型模式和模拟平台模式禁止启动外部插件，因此该接线
只有模拟与代码证据，尚无真实控制插件进程或 Windows 隔离证据。
