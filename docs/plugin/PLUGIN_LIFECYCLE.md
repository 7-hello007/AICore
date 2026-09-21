# Plugin Lifecycle

`Discover → Manifest Validate → Trust Validate → Capability Bind → Start → Health → Stop → Update/Rollback → Remove`

生命周期必须和业务执行解耦；Plugin 崩溃不能导致 Core 进程崩溃。
