use crate::capability::entry::CapabilityEntry;

/// 能力提供者抽象。
///
/// 任何 Adapter、Provider、Executor 都可以实现此 trait，
/// 用来声明自己支持的真实能力。
pub trait ICapabilityProvider: Send + Sync {
    /// 返回当前环境下真实支持的能力列表。
    ///
    /// 实现要求：
    /// - 必须返回至少一条能力。
    /// - 非 `Supported` / `ReadOnly` 的能力必须带 detail。
    /// - 能力列表应是确定性快照；不要在调用时修改内部状态。
    fn capabilities_detailed(&self) -> Vec<CapabilityEntry>;

    /// 组件名，例如 `linux_capability` / `windows_capability`。
    fn component_name(&self) -> &'static str;

    /// 便捷方法：只返回能力名。
    fn capability_names(&self) -> Vec<String> {
        self.capabilities_detailed()
            .into_iter()
            .map(|c| c.name)
            .collect()
    }

    /// 便捷方法：查询单个能力。
    fn capability(&self, name: &str) -> Option<CapabilityEntry> {
        self.capabilities_detailed()
            .into_iter()
            .find(|c| c.name == name)
    }
}
