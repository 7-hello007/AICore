//! L0 真实 workload 验证框架。
//!
//! 用合成 ComputerSnapshot 喂入分类器 + 策略引擎 + 执行器，
//! 验证"给定真实应用特征 → 分类器 → 策略 → 执行"完整链路。
//!
//! 用法：
//!   aicore-l0-realworld --output <path>
//!   aicore-l0-realworld --scenario <name>

mod evidence;
mod runner;
mod scenarios;

use std::process::ExitCode;
use std::time::{SystemTime, UNIX_EPOCH};

use evidence::RealWorldReport;

const HELP: &str = "\
aicore-l0-realworld — L0 real-world workload validation

USAGE:
    aicore-l0-realworld [OPTIONS]

OPTIONS:
    --output <PATH>         Write report JSON to path
    --scenario <NAME>       Run only one scenario
    --list                  List available scenarios
    -h, --help              Print help
";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();

    if args.iter().any(|a| a == "-h" || a == "--help") {
        println!("{}", HELP);
        return ExitCode::SUCCESS;
    }

    if args.iter().any(|a| a == "--list") {
        for s in scenarios::all_scenarios() {
            println!("{}  (expects {:?})", s.name, s.expected_workload);
        }
        return ExitCode::SUCCESS;
    }

    let output_path = args
        .iter()
        .position(|a| a == "--output")
        .and_then(|i| args.get(i + 1))
        .map(std::path::PathBuf::from);

    let filter = args
        .iter()
        .position(|a| a == "--scenario")
        .and_then(|i| args.get(i + 1));

    let started = now_unix_secs();

    let all = scenarios::all_scenarios();
    let selected: Vec<_> = match filter {
        Some(name) => {
            let filtered: Vec<_> = all.into_iter().filter(|s| s.name == name).collect();
            if filtered.is_empty() {
                eprintln!("error: unknown scenario `{}`", name);
                return ExitCode::from(2);
            }
            filtered
        },
        None => all,
    };

    let mut evidences = Vec::new();
    for scenario in &selected {
        let evidence = runner::run_scenario(scenario);
        let status = if evidence.passed { "PASS" } else { "FAIL" };
        println!(
            "[{}] {} inferred={} expected={}",
            status, scenario.name, evidence.inferred_workload, evidence.expected_workload
        );
        if !evidence.passed {
            for f in &evidence.failures {
                println!("    - {}", f);
            }
        }
        evidences.push(evidence);
    }

    let finished = now_unix_secs();
    let passed = evidences.iter().filter(|e| e.passed).count();

    let report = RealWorldReport {
        version: "V3.2".to_string(),
        started_at_unix_ms: started,
        finished_at_unix_ms: finished,
        scenarios_run: evidences.len(),
        scenarios_passed: passed,
        scenarios_failed: evidences.len() - passed,
        evidences,
    };

    if let Some(path) = output_path {
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        match serde_json::to_string_pretty(&report) {
            Ok(json) => {
                if let Err(e) = std::fs::write(&path, json) {
                    eprintln!("error: write {}: {}", path.display(), e);
                    return ExitCode::from(1);
                }
                eprintln!("wrote {}", path.display());
            },
            Err(e) => {
                eprintln!("error: serialize: {}", e);
                return ExitCode::from(1);
            },
        }
    }

    if report.all_passed() {
        ExitCode::SUCCESS
    } else {
        ExitCode::from(1)
    }
}

fn now_unix_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}
