use aicore_contracts::{
    context::workload::WorkloadType,
    telemetry::{
        application::ApplicationSnapshot, cpu::CpuSnapshot, gpu::GpuSnapshot,
        memory::MemorySnapshot, npu::NpuSnapshot, process::ProcessSnapshot,
        snapshot::ComputerSnapshot,
    },
};

/// L0 场景定义。
#[derive(Debug, Clone)]
pub struct Scenario {
    pub name: &'static str,
    /// 应用名（用于 foreground）。
    pub foreground_app: &'static str,
    /// 应用版本（人类可读；evidence 记录）。
    pub app_version: &'static str,
    /// 期望分类器识别的 workload。
    pub expected_workload: WorkloadType,
    /// 合成 snapshot 构造器。
    pub snapshot: fn() -> ComputerSnapshot,
}

pub fn all_scenarios() -> Vec<Scenario> {
    vec![
        Scenario {
            name: "development_vscode",
            foreground_app: "Code",
            app_version: "1.85",
            expected_workload: WorkloadType::Development,
            snapshot: snap_development,
        },
        Scenario {
            name: "development_neovim",
            foreground_app: "nvim",
            app_version: "0.10",
            expected_workload: WorkloadType::Development,
            snapshot: snap_development_neovim,
        },
        Scenario {
            name: "ai_local_ollama",
            foreground_app: "ollama",
            app_version: "0.3",
            expected_workload: WorkloadType::AiInference,
            snapshot: snap_ai_ollama,
        },
        Scenario {
            name: "office_browser",
            foreground_app: "chrome",
            app_version: "130",
            expected_workload: WorkloadType::GeneralOffice,
            snapshot: snap_office_browser,
        },
        Scenario {
            name: "rendering_blender",
            foreground_app: "blender",
            app_version: "4.2",
            expected_workload: WorkloadType::VideoEditing,
            snapshot: snap_rendering_blender,
        },
        Scenario {
            name: "video_conference",
            foreground_app: "zoom",
            app_version: "6.0",
            expected_workload: WorkloadType::VideoConference,
            snapshot: snap_video_conference,
        },
    ]
}

fn base_snapshot() -> ComputerSnapshot {
    ComputerSnapshot {
        cpu: CpuSnapshot {
            logical_cores: 8,
            per_core_usage: vec![10.0; 8],
            ..Default::default()
        },
        memory: MemorySnapshot {
            total_mb: 16384,
            used_mb: 8192,
            available_mb: 8192,
            usage_percent: 50.0,
            ..Default::default()
        },
        ..Default::default()
    }
}

fn with_foreground(
    mut snapshot: ComputerSnapshot,
    app_name: &str,
    pid: u32,
    window_title: &str,
) -> ComputerSnapshot {
    snapshot.applications = vec![ApplicationSnapshot {
        name: app_name.to_string(),
        pid,
        is_foreground: true,
        window_title: Some(window_title.to_string()),
    }];
    snapshot.top_processes = vec![ProcessSnapshot {
        pid,
        name: app_name.to_string(),
        cpu_usage_percent: 15.0,
        memory_mb: 512,
        is_foreground: true,
    }];
    snapshot
}

fn snap_development() -> ComputerSnapshot {
    let mut s = base_snapshot();
    s.cpu.usage_percent = 35.0;
    with_foreground(s, "Code", 1000, "main.rs - aicore")
}

fn snap_development_neovim() -> ComputerSnapshot {
    let mut s = base_snapshot();
    s.cpu.usage_percent = 25.0;
    with_foreground(s, "nvim", 1001, "init.lua")
}

fn snap_ai_ollama() -> ComputerSnapshot {
    let mut s = base_snapshot();
    s.cpu.usage_percent = 20.0;
    s.gpu = GpuSnapshot {
        name: Some("Test GPU".to_string()),
        usage_percent: Some(20.0),
        ..Default::default()
    };
    s.npu = NpuSnapshot {
        name: Some("accel0".to_string()),
        available: true,
        usage_percent: Some(90.0),
        ..Default::default()
    };
    with_foreground(s, "ollama", 2000, "ollama serve")
}

fn snap_office_browser() -> ComputerSnapshot {
    let mut s = base_snapshot();
    s.cpu.usage_percent = 12.0;
    with_foreground(s, "chrome", 3000, "Docs - Google Chrome")
}

fn snap_rendering_blender() -> ComputerSnapshot {
    let mut s = base_snapshot();
    s.cpu.usage_percent = 45.0;
    s.gpu = GpuSnapshot {
        name: Some("Test GPU".to_string()),
        usage_percent: Some(85.0),
        memory_used_mb: Some(4096),
        memory_total_mb: Some(8192),
        ..Default::default()
    };
    with_foreground(s, "blender", 4000, "scene.blend - Blender")
}

fn snap_video_conference() -> ComputerSnapshot {
    let mut s = base_snapshot();
    s.cpu.usage_percent = 30.0;
    with_foreground(s, "zoom", 5000, "Zoom Meeting")
}
