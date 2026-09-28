[英文](README.md) | [中文](README.zh-CN.md)

# AICore 基准测试

## V3.2 基线

| 文件 | 说明 |
|---|---|
| `v3.2-baseline.json` | 五百个周期的延迟分布、常驻内存和线程数基线 |

## 生成方式

```bash
cargo build -p aicore-bench
./target/debug/aicore-bench --ticks 500 --output benchmarks/v3.2-baseline.json
```

## 指标

| 字段 | 说明 |
|---|---|
| `p50_tick_ms` / `p95_tick_ms` / `p99_tick_ms` | 周期延迟分布 |
| `rss_mb_before` / `rss_mb_after` | 运行前后的常驻内存 |
| `rss_growth_mb` | 常驻内存增长量，不应超过 32 MB |
| `threads_before` / `threads_after` | 线程数 |

## 回归门槛

在相同硬件和相同配置下对比结果：

- `p99_tick_ms` 增长不得超过两倍。
- `rss_growth_mb` 增长不得超过两倍。
- `threads_after` 增长不得超过八个线程。
