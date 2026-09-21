//! Plugin C ABI。
//!
//! 插件必须导出三个符号：
//! - `aicore_plugin_api_version: extern "C" fn() -> u32`
//! - `aicore_plugin_name: extern "C" fn() -> *const c_char`
//! - `aicore_plugin_execute: extern "C" fn(u32) -> u32`

use std::os::raw::c_char;

/// 当前 ABI 版本。
pub const API_VERSION: u32 = 1;

pub type VersionFn = unsafe extern "C" fn() -> u32;
pub type NameFn = unsafe extern "C" fn() -> *const c_char;
pub type ExecuteFn = unsafe extern "C" fn(input: u32) -> u32;

pub const SYM_VERSION: &[u8] = b"aicore_plugin_api_version\0";
pub const SYM_NAME: &[u8] = b"aicore_plugin_name\0";
pub const SYM_EXECUTE: &[u8] = b"aicore_plugin_execute\0";
