use crate::snapshot::DashboardSnapshot;

/// Render snapshots as strings.
///
/// Pure function, no side effects, width adjusts.
pub fn render(snapshot: &DashboardSnapshot, width: usize) -> String {
    let w = width.max(60);
    let mut out = String::new();
    out.push_str(&render_header(snapshot, w));
    out.push_str(&render_status(snapshot, w));
    out.push_str(&render_last_tick(snapshot, w));
    out.push_str(&render_runtime_route(snapshot, w));
    out.push_str(&render_recent_ticks(snapshot, w));
    out.push_str(&render_capabilities(snapshot, w));
    out.push_str(&render_plugins(snapshot, w));
    if !snapshot.tier_states.is_empty() {
        out.push_str(&render_tiers(snapshot, w));
    }
    if snapshot.retention_status.is_some() {
        out.push_str(&render_retention(snapshot, w));
    }
    if snapshot.ai_runtime_health.is_some() {
        out.push_str(&render_ai_runtime(snapshot, w));
    }
    if !snapshot.errors.is_empty() {
        out.push_str(&render_errors(snapshot, w));
    }
    out.push_str(&render_footer(snapshot, w));
    out
}

fn render_runtime_route(s: &DashboardSnapshot, _w: usize) -> String {
    let mut out = section("Runtime Route");
    match &s.runtime_route {
        None => out.push_str("  (no route recorded)\n"),
        Some(r) => {
            out.push_str(&line("source", r.source.as_str()));
            out.push_str(&line("applied", &r.applied.to_string()));
            out.push_str(&line("confidence", &format!("{:.3}", r.confidence)));
            out.push_str(&line("reason", &r.reason));
        },
    }
    out.push('\n');
    out
}

fn render_tiers(s: &DashboardSnapshot, _w: usize) -> String {
    let mut out = section("Tiers");
    for t in &s.tier_states {
        out.push_str(&format!("  {:<16} {:<6} {}\n", t.id, t.level, t.state));
    }
    out.push('\n');
    out
}

fn render_retention(s: &DashboardSnapshot, _w: usize) -> String {
    let mut out = section("Retention");
    if let Some(r) = &s.retention_status {
        out.push_str(&line("enabled", &r.enabled.to_string()));
        out.push_str(&line("retention_days", &r.retention_days.to_string()));
        out.push_str(&line(
            "removed_last",
            &r.records_removed_last_run.to_string(),
        ));
        out.push_str(&line("last_run", &format_unix(r.last_run_unix_secs)));
    }
    out.push('\n');
    out
}

fn render_ai_runtime(s: &DashboardSnapshot, _w: usize) -> String {
    let mut out = section("AI Runtime");
    if let Some(h) = &s.ai_runtime_health {
        out.push_str(&line("runtime", &h.runtime_name));
        out.push_str(&line("healthy", &h.healthy.to_string()));
        out.push_str(&line("loaded_models", &h.loaded_models.to_string()));
        if let Some(reason) = &h.degraded_reason {
            out.push_str(&line("degraded", reason));
        }
    }
    out.push('\n');
    out
}

fn render_header(s: &DashboardSnapshot, w: usize) -> String {
    let bar = "═".repeat(w);
    let ts = format_unix(s.fetched_at_unix_ms / 1000);
    let mut out = String::new();
    out.push_str(&bar);
    out.push('\n');
    out.push_str(&format!(
        " AICore Dashboard{:>width$}\n",
        ts,
        width = w.saturating_sub(17)
    ));
    out.push_str(&bar);
    out.push('\n');
    out
}

fn render_status(s: &DashboardSnapshot, w: usize) -> String {
    let mut out = section("Status");
    match &s.daemon_status {
        None => out.push_str("  (daemon unavailable)\n"),
        Some(st) => {
            out.push_str(&line("pid", &st.pid.to_string()));
            out.push_str(&line("tick count", &st.tick_count.to_string()));
            out.push_str(&line(
                "success / fail",
                &format!("{} / {}", st.successful_ticks, st.failed_ticks),
            ));
            out.push_str(&line("mode", &format!("{:?}", st.current_mode)));
            out.push_str(&line("last tick at", &format_unix(st.last_tick_at_unix_ms)));
        },
    }
    let _ = w;
    out.push('\n');
    out
}

fn render_last_tick(s: &DashboardSnapshot, _w: usize) -> String {
    let mut out = section("Last Tick");
    let tick = s
        .daemon_status
        .as_ref()
        .and_then(|st| st.last_tick.as_ref());
    match tick {
        None => out.push_str("  (no tick recorded yet)\n"),
        Some(t) => {
            out.push_str(&line("tick_id", &t.tick_id.to_string()));
            out.push_str(&line("trace_id", &t.trace_id));
            out.push_str(&line("duration", &format!("{} ms", t.duration_ms)));
            out.push_str(&line("success", &t.success.to_string()));
            out.push_str(&line("workload", &t.workload_type));
            if let Some(p) = &t.policy_id {
                out.push_str(&line("policy_id", p));
            }
            if let Some(src) = &t.policy_source {
                out.push_str(&line("policy_source", &format!("{:?}", src)));
            }
            if let Some(es) = &t.execution_status {
                out.push_str(&line("execution", &format!("{:?}", es)));
            }
            if let Some(pw) = &t.predicted_workload {
                out.push_str(&line("predicted", &format!("{:?}", pw)));
            }
            if let Some(c) = t.prediction_confidence {
                out.push_str(&line("pred_conf", &format!("{:.3}", c)));
            }
        },
    }
    out.push('\n');
    out
}

fn render_recent_ticks(s: &DashboardSnapshot, _w: usize) -> String {
    let mut out = section("Recent Ticks");
    if s.recent_ticks.is_empty() {
        out.push_str("  (none)\n\n");
        return out;
    }
    out.push_str(&format!(
        "  {:>6}  {:>8}  {:>7}  {:<16}  {:<14}  {}\n",
        "tick", "duration", "success", "workload", "source", "predicted"
    ));
    for t in &s.recent_ticks {
        let src = t
            .policy_source
            .as_ref()
            .map(|s| format!("{:?}", s))
            .unwrap_or_else(|| "-".to_string());
        let pred = match (&t.predicted_workload, t.prediction_confidence) {
            (Some(w), Some(c)) => format!("{:?} ({:.2})", w, c),
            (Some(w), None) => format!("{:?}", w),
            _ => "-".to_string(),
        };
        out.push_str(&format!(
            "  {:>6}  {:>6}ms  {:>7}  {:<16}  {:<14}  {}\n",
            t.tick_id, t.duration_ms, t.success, t.workload_type, src, pred
        ));
    }
    out.push('\n');
    out
}

fn render_capabilities(s: &DashboardSnapshot, _w: usize) -> String {
    let mut out = section("Capabilities");
    if s.capabilities.is_empty() {
        out.push_str("  (none)\n\n");
        return out;
    }
    for c in &s.capabilities {
        let status = format!("{:?}", c.status);
        match &c.detail {
            Some(d) => out.push_str(&format!("  [{:<13}] {:<32} {}\n", status, c.name, d)),
            None => out.push_str(&format!("  [{:<13}] {}\n", status, c.name)),
        }
    }
    out.push('\n');
    out
}

fn render_plugins(s: &DashboardSnapshot, _w: usize) -> String {
    let mut out = section("Plugins");
    if s.plugin_states.is_empty() {
        out.push_str("  (none registered)\n\n");
        return out;
    }
    for p in &s.plugin_states {
        out.push_str(&format!(
            "  {:<24} v{:<10} {:<10} {:<12} {}\n",
            p.name, p.version, p.kind, p.trust_level, p.state
        ));
        if !p.interfaces.is_empty() {
            out.push_str(&format!("      interfaces: {}\n", p.interfaces.join(", ")));
        }
        if let Some(r) = &p.last_reason {
            out.push_str(&format!("      reason:     {}\n", r));
        }
    }
    out.push('\n');
    out
}

fn render_errors(s: &DashboardSnapshot, _w: usize) -> String {
    let mut out = section("Errors");
    for e in &s.errors {
        out.push_str(&format!("  ! {}\n", e));
    }
    out.push('\n');
    out
}

fn render_footer(s: &DashboardSnapshot, w: usize) -> String {
    let bar = "─".repeat(w);
    let summary = if s.errors.is_empty() {
        "OK".to_string()
    } else {
        format!("{} errors", s.errors.len())
    };
    format!(
        "{}\n  status: {}   (press Ctrl+C to exit)\n{}\n",
        bar, summary, bar
    )
}

fn section(title: &str) -> String {
    format!("[ {} ]\n", title)
}

fn line(key: &str, value: &str) -> String {
    format!("  {:<16} {}\n", key, value)
}

fn format_unix(secs: u64) -> String {
    if secs == 0 {
        return "-".to_string();
    }
    format!("{}", secs)
}
