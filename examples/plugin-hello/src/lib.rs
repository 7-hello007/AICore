//! 示例 cdylib 插件：导出 AICore Plugin ABI 要求的 3 个符号。

use std::os::raw::c_char;

const API_VERSION: u32 = 1;
const NAME: &[u8] = b"plugin-hello\0";

#[no_mangle]
pub extern "C" fn aicore_plugin_api_version() -> u32 {
    API_VERSION
}

#[no_mangle]
pub extern "C" fn aicore_plugin_name() -> *const c_char {
    NAME.as_ptr() as *const c_char
}

/// 简单执行：返回输入 × 2 + 1。
#[no_mangle]
pub extern "C" fn aicore_plugin_execute(input: u32) -> u32 {
    input.wrapping_mul(2).wrapping_add(1)
}
