# AICore Benchmarks

## V3.2 baseline

| 文件 | 说明 |
|---|---|
| `v3.2-baseline.json` | 500 ticks 的延迟分布 + RSS + 线程数基线 |

## 生成

```bash
cargo build -p aicore-bench
./target/debug/aicore-bench --ticks 500 --output benchmarks/v3.2-baseline.json
指标
字段	说明
p50_tick_ms / p95_tick_ms / p99_tick_ms	tick 延迟分布
rss_mb_before / rss_mb_after	前后 RSS
rss_growth_mb	RSS 增长（应 ≤ 32MB）
threads_before / threads_after	线程数
Regression Gate
同硬件同配置下，与基线对比：

p99_tick_ms 增长 ≤ 2x

rss_growth_mb 增长 ≤ 2x

threads_after 增长 ≤ +8
