[English](README.md) | [Chinese](README.zh-CN.md)

# AICore Benchmarks

## V3.2 Baseline

| File | Description |
|---|---|
| `v3.2-baseline.json` | Baseline for the latency distribution, resident memory, and thread count across 500 ticks |

## Generation

```bash
cargo build -p aicore-bench
./target/debug/aicore-bench --ticks 500 --output benchmarks/v3.2-baseline.json
```

## Metrics

| Field | Description |
|---|---|
| `p50_tick_ms` / `p95_tick_ms` / `p99_tick_ms` | Tick latency distribution |
| `rss_mb_before` / `rss_mb_after` | Resident memory before and after the run |
| `rss_growth_mb` | Resident memory growth, which should not exceed 32 MB |
| `threads_before` / `threads_after` | Thread count |

## Regression Gate

Compare results on the same hardware with the same configuration:

- `p99_tick_ms` growth must not exceed 2x.
- `rss_growth_mb` growth must not exceed 2x.
- `threads_after` growth must not exceed 8 threads.
