# Plugin Lifecycle

`Discover → Manifest Validate → Trust Validate → Capability Bind → Start → Health → Stop → Update/Rollback → Remove`

生命周期必须和业务执行解耦；Plugin 崩溃不能导致 Core 进程崩溃。

## 当前代码状态

`PluginActivationManager` 仅在操作员通过 `--plugin-policy` 指定的 pin 中找到
唯一匹配项时尝试激活。pin 包含绝对 manifest/artifact 路径、两份 SHA-256、
trust level、可选 Ed25519 签名与公钥、允许接口/权限和进程预算。激活前重读并
核对 manifest 与 artifact，随后把已验证字节放入私有暂存目录；进程完成
Ping/Pong 后才进入 Active。健康轮询发现崩溃时解绑并标记 Failed，stop/Drop
会终止受管进程。Model Mode 禁止外部插件激活。

目前只绑定现有 `extension.execute` 协议；GPU/遥测等 typed interface 尚无
进程协议适配。自动重启、升级和 rollback 尚未实现，不能把上述生命周期
描述当作全部验收通过。Windows 进程隔离还需要原生运行验证。
