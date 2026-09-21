use aicore_contracts::{
    context::workload::WorkloadType,
    traits::task_classifier::{ClassificationInput, ITaskClassifier},
};

/// 基于规则的 workload 分类器。
///
/// 规则优先、确定性、无外部模型依赖。匹配采用标准化 token / phrase，
/// 避免 `contains` 导致短词误命中长进程名。
pub struct RuleBasedClassifier;

impl RuleBasedClassifier {
    pub fn new() -> Self {
        Self
    }
}

impl Default for RuleBasedClassifier {
    fn default() -> Self {
        Self::new()
    }
}

impl ITaskClassifier for RuleBasedClassifier {
    fn classify(&self, input: &ClassificationInput) -> WorkloadType {
        if let Some(app) = &input.foreground_app {
            if let Some(w) = classify_by_name(app) {
                return w;
            }
        }
        if let Some(proc_name) = &input.foreground_process {
            if let Some(w) = classify_by_name(proc_name) {
                return w;
            }
        }
        for name in &input.active_process_names {
            if let Some(w) = classify_by_name(name) {
                return w;
            }
        }

        if let Some(npu) = input.npu_usage_percent {
            if npu > 20.0 {
                return WorkloadType::AiInference;
            }
        }
        if let Some(gpu) = input.gpu_usage_percent {
            if gpu > 70.0 {
                return WorkloadType::Rendering;
            }
        }
        if input.cpu_usage_percent > 70.0 && input.memory_usage_percent > 45.0 {
            return WorkloadType::Compilation;
        }
        WorkloadType::Unknown
    }

    fn confidence(&self, input: &ClassificationInput) -> f32 {
        if input
            .foreground_app
            .as_deref()
            .and_then(classify_by_name)
            .is_some()
        {
            return 0.95;
        }
        if input
            .foreground_process
            .as_deref()
            .and_then(classify_by_name)
            .is_some()
        {
            return 0.90;
        }
        if input
            .active_process_names
            .iter()
            .any(|n| classify_by_name(n).is_some())
        {
            return 0.82;
        }
        if input.npu_usage_percent.map(|v| v > 20.0).unwrap_or(false) {
            return 0.80;
        }
        if input.gpu_usage_percent.map(|v| v > 70.0).unwrap_or(false) {
            return 0.72;
        }
        if input.cpu_usage_percent > 70.0 && input.memory_usage_percent > 45.0 {
            return 0.68;
        }
        0.0
    }

    fn name(&self) -> &'static str {
        "rule_based"
    }
}

fn normalize(value: &str) -> String {
    value
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() {
                c.to_ascii_lowercase()
            } else {
                ' '
            }
        })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn has_phrase(normalized: &str, phrase: &str) -> bool {
    let hay = format!(" {} ", normalized);
    let needle = format!(" {} ", normalize(phrase));
    hay.contains(&needle)
}

fn classify_by_name(name: &str) -> Option<WorkloadType> {
    let normalized = normalize(name);
    if normalized.is_empty() {
        return None;
    }

    const DEV: &[&str] = &[
        "visual studio code",
        "code",
        "vscode",
        "cursor",
        "idea",
        "pycharm",
        "rustrover",
        "goland",
        "clion",
        "webstorm",
        "vim",
        "nvim",
        "neovim",
        "emacs",
        "terminal",
        "gnome terminal",
        "konsole",
        "alacritty",
        "kitty",
        "wezterm",
        "tmux",
        "bash",
        "zsh",
        "fish",
        "powershell",
        "windows terminal",
        "cargo",
        "rustc",
    ];
    if DEV.iter().any(|k| has_phrase(&normalized, k)) {
        return Some(WorkloadType::Development);
    }

    const AI: &[&str] = &[
        "ollama",
        "llama cpp",
        "llama",
        "lmstudio",
        "lm studio",
        "vllm",
        "text generation",
        "stable diffusion",
        "comfy",
        "automatic1111",
        "sillytavern",
        "koboldcpp",
        "whisper",
    ];
    if AI.iter().any(|k| has_phrase(&normalized, k)) {
        return Some(WorkloadType::AiInference);
    }

    const GAME: &[&str] = &[
        "steam",
        "epic games",
        "riot",
        "valorant",
        "csgo",
        "counter strike",
        "dota",
        "league of legends",
        "leagueclient",
        "battle net",
        "battlenet",
        "genshin",
        "elden ring",
        "minecraft",
        "fortnite",
    ];
    if GAME.iter().any(|k| has_phrase(&normalized, k)) {
        return Some(WorkloadType::Gaming);
    }

    const VIDEO: &[&str] = &[
        "blender",
        "premiere",
        "davinci",
        "resolve",
        "final cut",
        "kdenlive",
        "shotcut",
        "after effects",
        "vegas",
    ];
    if VIDEO.iter().any(|k| has_phrase(&normalized, k)) {
        return Some(WorkloadType::VideoEditing);
    }

    const RENDER: &[&str] = &["c4d", "maya", "3dsmax", "houdini", "octane", "cycles"];
    if RENDER.iter().any(|k| has_phrase(&normalized, k)) {
        return Some(WorkloadType::Rendering);
    }

    const CONF: &[&str] = &["zoom", "teams", "webex", "google meet", "meet", "skype"];
    if CONF.iter().any(|k| has_phrase(&normalized, k)) {
        return Some(WorkloadType::VideoConference);
    }

    const OFFICE: &[&str] = &[
        "word",
        "excel",
        "powerpoint",
        "pages",
        "numbers",
        "keynote",
        "chrome",
        "firefox",
        "edge",
        "safari",
        "slack",
        "discord",
        "telegram",
        "notion",
        "obsidian",
        "typora",
    ];
    if OFFICE.iter().any(|k| has_phrase(&normalized, k)) {
        return Some(WorkloadType::GeneralOffice);
    }

    const BUILD: &[&str] = &[
        "make", "ninja", "cmake", "gradle", "maven", "bazel", "msbuild",
    ];
    if BUILD.iter().any(|k| has_phrase(&normalized, k)) {
        return Some(WorkloadType::Compilation);
    }

    const FILE: &[&str] = &["7z", "winrar", "tar", "zip", "unzip", "ffmpeg"];
    if FILE.iter().any(|k| has_phrase(&normalized, k)) {
        return Some(WorkloadType::FileProcessing);
    }

    None
}
