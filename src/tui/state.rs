//! TUI application state types.

use std::time::Instant;

#[cfg(any(target_os = "windows", target_os = "linux"))]
use std::sync::mpsc;

#[cfg(any(target_os = "windows", target_os = "linux"))]
use crate::benchmark_runner;
use crate::config::Config;
use crate::feedback::{self, FeedbackSurface};
use crate::games::KNOWN_GAMES;
use crate::hardware::SystemInfo;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Screen {
    Home,
    Contribute(ContributeStep),
    ContributeResult,
    Feedback(FeedbackStep),
    FeedbackResult,
    #[cfg(any(target_os = "windows", target_os = "linux"))]
    SyntheticRunning,
    #[cfg(any(target_os = "windows", target_os = "linux"))]
    SyntheticResult,
    ErrorModal,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum FeedbackStep {
    Category,
    Issue,
    Message,
    Submitting,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ContributeStep {
    Consent,
    Hardware,
    Baseline,
    Game,
    Results,
    Review,
    Submitting,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum HomeChoice {
    GuidedFlow,
    #[cfg(any(target_os = "windows", target_os = "linux"))]
    Synthetic,
    Feedback,
    Quit,
}

#[derive(Debug, Clone)]
pub(crate) struct FeedbackDraft {
    pub category_index: usize,
    pub issue_index: usize,
    pub message: String,
    pub include_diagnostics: bool,
}

impl FeedbackDraft {
    pub fn new() -> Self {
        Self {
            category_index: 0,
            issue_index: 0,
            message: String::new(),
            include_diagnostics: false,
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct FeedbackResultState {
    pub title: String,
    pub body: String,
}

#[derive(Debug, Clone)]
pub(crate) struct MessageResultState {
    pub title: String,
    pub body: String,
}

#[cfg(any(target_os = "windows", target_os = "linux"))]
#[derive(Debug)]
pub(crate) enum SyntheticWorkerEvent {
    Progress(benchmark_runner::BenchmarkProgressUpdate),
    Finished(Box<anyhow::Result<benchmark_runner::BenchmarkResults>>),
}

#[cfg(any(target_os = "windows", target_os = "linux"))]
#[derive(Debug)]
pub(crate) struct SyntheticState {
    pub started_at: Instant,
    pub rx: mpsc::Receiver<SyntheticWorkerEvent>,
}

#[cfg(target_os = "windows")]
#[derive(Debug)]
pub(crate) struct PresentmonInstallState {
    pub rx: mpsc::Receiver<anyhow::Result<Option<std::path::PathBuf>>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SyntheticReturn {
    Home,
    Contribute,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CaptureMethodChoice {
    InGameCounter,
    BuiltInBenchmark,
    ExternalTool,
}

impl CaptureMethodChoice {
    pub fn as_api_value(&self) -> &'static str {
        match self {
            CaptureMethodChoice::InGameCounter => "in_game_counter",
            CaptureMethodChoice::BuiltInBenchmark => "built_in_benchmark",
            CaptureMethodChoice::ExternalTool => "external_tool",
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            CaptureMethodChoice::InGameCounter => "In-game counter",
            CaptureMethodChoice::BuiltInBenchmark => "Built-in benchmark",
            CaptureMethodChoice::ExternalTool => "External tool (PresentMon/MangoHud/etc.)",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum InputMode {
    Navigate,
    Edit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum HardwareField {
    GpuName,
    GpuVramMb,
    CpuName,
    CpuCores,
    CpuThreads,
    RamTotalMb,
}

impl HardwareField {
    pub const ALL: [HardwareField; 6] = [
        HardwareField::GpuName,
        HardwareField::GpuVramMb,
        HardwareField::CpuName,
        HardwareField::CpuCores,
        HardwareField::CpuThreads,
        HardwareField::RamTotalMb,
    ];
}

#[derive(Debug, Clone)]
pub(crate) struct HardwareForm {
    pub info: SystemInfo,
    pub gpu_name: String,
    pub gpu_vram_mb: String,
    pub cpu_name: String,
    pub cpu_cores: String,
    pub cpu_threads: String,
    pub ram_total_mb: String,
    pub confirm_gpu: bool,
    pub confirm_cpu: bool,
    pub confirm_ram: bool,
    pub field: HardwareField,
    pub mode: InputMode,
}

impl HardwareForm {
    pub fn from_info(info: SystemInfo) -> Self {
        let gpu_vram_mb = info.gpu.vram_mb.map(|v| v.to_string()).unwrap_or_default();
        let ram_mb = info.ram.installed_mb.unwrap_or(info.ram.usable_mb);

        Self {
            gpu_name: info.gpu.name.clone(),
            gpu_vram_mb,
            cpu_name: info.cpu.name.clone(),
            cpu_cores: info.cpu.cores.to_string(),
            cpu_threads: info.cpu.threads.to_string(),
            ram_total_mb: ram_mb.to_string(),
            info,
            confirm_gpu: false,
            confirm_cpu: false,
            confirm_ram: false,
            field: HardwareField::GpuName,
            mode: InputMode::Navigate,
        }
    }

    pub fn has_required_values(&self) -> bool {
        let gpu_name_ok = !self.gpu_name.trim().is_empty();
        let cpu_name_ok = !self.cpu_name.trim().is_empty();
        let cores_ok = self.cpu_cores.trim().parse::<usize>().unwrap_or(0) > 0;
        let threads_ok = self.cpu_threads.trim().parse::<usize>().unwrap_or(0) > 0;
        let ram_ok = self.ram_total_mb.trim().parse::<u64>().unwrap_or(0) > 0;
        gpu_name_ok && cpu_name_ok && cores_ok && threads_ok && ram_ok
    }

    pub fn all_confirmed(&self) -> bool {
        self.confirm_gpu && self.confirm_cpu && self.confirm_ram
    }

    pub fn can_continue(&self) -> bool {
        self.has_required_values() && self.all_confirmed()
    }

    pub fn active_value_mut(&mut self) -> &mut String {
        match self.field {
            HardwareField::GpuName => &mut self.gpu_name,
            HardwareField::GpuVramMb => &mut self.gpu_vram_mb,
            HardwareField::CpuName => &mut self.cpu_name,
            HardwareField::CpuCores => &mut self.cpu_cores,
            HardwareField::CpuThreads => &mut self.cpu_threads,
            HardwareField::RamTotalMb => &mut self.ram_total_mb,
        }
    }

    pub fn field_index(&self) -> usize {
        HardwareField::ALL
            .iter()
            .position(|f| *f == self.field)
            .unwrap_or(0)
    }

    pub fn set_field_by_index(&mut self, idx: usize) {
        self.field = HardwareField::ALL[idx.min(HardwareField::ALL.len().saturating_sub(1))];
    }

    pub fn next_field(&mut self) {
        let idx = (self.field_index() + 1) % HardwareField::ALL.len();
        self.set_field_by_index(idx);
    }

    pub fn prev_field(&mut self) {
        let idx = self.field_index();
        let next = if idx == 0 {
            HardwareField::ALL.len().saturating_sub(1)
        } else {
            idx - 1
        };
        self.set_field_by_index(next);
    }

    pub fn commit_into_info(&mut self) {
        self.info.gpu.name = self.gpu_name.trim().to_string();
        self.info.cpu.name = self.cpu_name.trim().to_string();
        if let Ok(vram) = self.gpu_vram_mb.trim().parse::<u64>() {
            if vram > 0 {
                self.info.gpu.vram_mb = Some(vram);
            }
        }
        if let Ok(cores) = self.cpu_cores.trim().parse::<usize>() {
            if cores > 0 {
                self.info.cpu.cores = cores;
            }
        }
        if let Ok(threads) = self.cpu_threads.trim().parse::<usize>() {
            if threads > 0 {
                self.info.cpu.threads = threads;
            }
        }
        if let Ok(ram_mb) = self.ram_total_mb.trim().parse::<u64>() {
            if ram_mb > 0 {
                self.info.ram.installed_mb = Some(ram_mb);
                self.info.ram.usable_mb = ram_mb;
            }
        }
    }
}

#[cfg(any(target_os = "windows", target_os = "linux"))]
#[derive(Debug)]
pub(crate) struct DetectState {
    pub started_at: Instant,
    pub rx: mpsc::Receiver<anyhow::Result<SystemInfo>>,
}

#[derive(Debug, Clone)]
pub(crate) struct ConsentState {
    pub tos: bool,
    pub public_use: bool,
    pub retention: bool,
    pub cursor: usize,
    pub scroll_offset: u16,
}

impl ConsentState {
    pub fn from_config(cfg: &Config) -> Self {
        Self {
            tos: cfg.consent.tos_accepted,
            public_use: cfg.consent.consent_public_use,
            retention: cfg.consent.retention_acknowledged,
            cursor: 0,
            scroll_offset: 0,
        }
    }

    pub fn is_complete(&self) -> bool {
        self.tos && self.public_use && self.retention
    }

    pub fn toggle_current(&mut self) {
        match self.cursor {
            0 => self.tos = !self.tos,
            1 => self.public_use = !self.public_use,
            _ => self.retention = !self.retention,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub(crate) struct GameState {
    pub query: String,
    pub cursor: usize,
}

#[derive(Debug, Clone)]
pub(crate) struct ResultsState {
    pub resolution: String,
    pub preset: String,
    pub avg_fps: String,
    pub fps_1_low: String,
    pub fps_01_low: String,
    pub ray_tracing: bool,
    pub upscaling: String,
    pub capture_method: CaptureMethodChoice,
    pub anti_cheat_ack: bool,
    pub cursor: usize,
    pub mode: InputMode,
}

impl Default for ResultsState {
    fn default() -> Self {
        Self {
            resolution: "1440p".to_string(),
            preset: "High".to_string(),
            avg_fps: String::new(),
            fps_1_low: String::new(),
            fps_01_low: String::new(),
            ray_tracing: false,
            upscaling: String::new(),
            capture_method: CaptureMethodChoice::BuiltInBenchmark,
            anti_cheat_ack: false,
            cursor: 0,
            mode: InputMode::Navigate,
        }
    }
}

#[derive(Debug)]
pub(crate) struct ContributeState {
    pub consent: ConsentState,
    pub hardware: Option<HardwareForm>,
    #[cfg(any(target_os = "windows", target_os = "linux"))]
    pub detect: Option<DetectState>,
    #[cfg(any(target_os = "windows", target_os = "linux"))]
    pub baseline: Option<benchmark_runner::BenchmarkResults>,
    pub game: GameState,
    pub selected_game: Option<usize>,
    pub results: ResultsState,
    pub result_message: Option<MessageResultState>,
    pub review_expanded: [bool; 3],
}

impl ContributeState {
    pub fn new(cfg: &Config) -> Self {
        Self {
            consent: ConsentState::from_config(cfg),
            hardware: None,
            #[cfg(any(target_os = "windows", target_os = "linux"))]
            detect: None,
            #[cfg(any(target_os = "windows", target_os = "linux"))]
            baseline: None,
            game: GameState::default(),
            selected_game: None,
            results: ResultsState::default(),
            result_message: None,
            review_expanded: [true, true, true],
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct ErrorModalState {
    pub title: String,
    pub message: String,
    pub kind: ModalKind,
    #[cfg_attr(not(target_os = "windows"), allow(dead_code))]
    pub confirm_action: Option<ConfirmAction>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(not(target_os = "windows"), allow(dead_code))]
pub(crate) enum ModalKind {
    Error,
    Confirm,
    Info,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ConfirmAction {
    #[cfg(target_os = "windows")]
    InstallPresentmon,
}

#[derive(Debug)]
pub(crate) enum Action {
    SubmitFeedback,
    SubmitBenchmark,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TuiExit {
    Quit,
}

pub(crate) struct AnimationState {
    pub tick: u64,
}

impl AnimationState {
    pub fn new() -> Self {
        Self { tick: 0 }
    }

    pub fn advance(&mut self) {
        self.tick = self.tick.wrapping_add(1);
    }

    pub fn spinner_char(&self) -> char {
        const FRAMES: [char; 10] = ['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧', '⠇', '⠏'];
        FRAMES[(self.tick as usize / 6) % FRAMES.len()]
    }
}

pub(crate) struct App {
    pub screen: Screen,
    pub schema: feedback::FeedbackSchema,
    pub home_choice: HomeChoice,
    pub feedback: FeedbackDraft,
    pub feedback_result: Option<FeedbackResultState>,
    pub contribute: ContributeState,
    pub synthetic_return: SyntheticReturn,
    pub synthetic_return_screen: Screen,
    #[cfg(any(target_os = "windows", target_os = "linux"))]
    pub synthetic: Option<SyntheticState>,
    #[cfg(any(target_os = "windows", target_os = "linux"))]
    pub synthetic_result: Option<benchmark_runner::BenchmarkResults>,
    #[cfg(any(target_os = "windows", target_os = "linux"))]
    pub synthetic_error: Option<String>,
    #[cfg(any(target_os = "windows", target_os = "linux"))]
    pub synthetic_progress: Option<benchmark_runner::BenchmarkProgressUpdate>,
    pub error_modal: Option<ErrorModalState>,
    pub error_return_screen: Screen,
    #[cfg(target_os = "windows")]
    pub presentmon_install: Option<PresentmonInstallState>,
    pub pending_action: Option<Action>,
    pub exit: Option<TuiExit>,
    pub last_tick: Instant,
    pub animation: AnimationState,
}

impl App {
    pub fn new() -> Self {
        let cfg = Config::load().unwrap_or_default();
        let app = Self {
            screen: Screen::Home,
            schema: feedback::schema_for(FeedbackSurface::TerminalUi),
            home_choice: HomeChoice::GuidedFlow,
            feedback: FeedbackDraft::new(),
            feedback_result: None,
            contribute: ContributeState::new(&cfg),
            synthetic_return: SyntheticReturn::Home,
            synthetic_return_screen: Screen::Home,
            #[cfg(any(target_os = "windows", target_os = "linux"))]
            synthetic: None,
            #[cfg(any(target_os = "windows", target_os = "linux"))]
            synthetic_result: None,
            #[cfg(any(target_os = "windows", target_os = "linux"))]
            synthetic_error: None,
            #[cfg(any(target_os = "windows", target_os = "linux"))]
            synthetic_progress: None,
            error_modal: None,
            error_return_screen: Screen::Home,
            #[cfg(target_os = "windows")]
            presentmon_install: None,
            pending_action: None,
            exit: None,
            last_tick: Instant::now(),
            animation: AnimationState::new(),
        };

        // Windows-only: PresentMon enables live auto-capture. Offer an opt-in install prompt once
        // at startup, since it unlocks key functionality and avoids later surprises.
        #[cfg(target_os = "windows")]
        {
            let mut app = app;
            let skip = std::env::var("FPS_TRACKER_SKIP_STARTUP_DEPS_PROMPT")
                .ok()
                .as_deref()
                .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
                .unwrap_or(false);
            if !skip && crate::deps::locate_presentmon_executable().is_none() {
                app.set_confirm(
                    "Install PresentMon",
                    "PresentMon enables Windows live auto-capture (frametime capture).\n\nInstall now? (Recommended)\n\nTip: if installation fails due to permissions, re-run the terminal as Administrator.",
                    ConfirmAction::InstallPresentmon,
                );
            }
            app
        }

        #[cfg(not(target_os = "windows"))]
        {
            app
        }
    }

    pub fn category(&self) -> &feedback::FeedbackCategorySchema {
        &self.schema.categories[self.feedback.category_index]
    }

    pub fn issue(&self) -> &feedback::FeedbackIssueOption {
        let issues = &self.category().issues;
        let idx = self
            .feedback
            .issue_index
            .min(issues.len().saturating_sub(1));
        &issues[idx]
    }

    pub fn set_error(&mut self, title: impl Into<String>, message: impl Into<String>) {
        self.error_return_screen = self.screen;
        self.error_modal = Some(ErrorModalState {
            title: title.into(),
            message: message.into(),
            kind: ModalKind::Error,
            confirm_action: None,
        });
        self.screen = Screen::ErrorModal;
    }

    #[cfg_attr(not(target_os = "windows"), allow(dead_code))]
    pub fn set_info(&mut self, title: impl Into<String>, message: impl Into<String>) {
        self.error_return_screen = self.screen;
        self.error_modal = Some(ErrorModalState {
            title: title.into(),
            message: message.into(),
            kind: ModalKind::Info,
            confirm_action: None,
        });
        self.screen = Screen::ErrorModal;
    }

    #[cfg_attr(not(target_os = "windows"), allow(dead_code))]
    pub fn set_confirm(
        &mut self,
        title: impl Into<String>,
        message: impl Into<String>,
        action: ConfirmAction,
    ) {
        self.error_return_screen = self.screen;
        self.error_modal = Some(ErrorModalState {
            title: title.into(),
            message: message.into(),
            kind: ModalKind::Confirm,
            confirm_action: Some(action),
        });
        self.screen = Screen::ErrorModal;
    }
}

pub(crate) fn filtered_games(query: &str) -> Vec<usize> {
    let q = query.trim().to_lowercase();
    KNOWN_GAMES
        .iter()
        .enumerate()
        .filter(|(_, g)| {
            if q.is_empty() {
                return true;
            }
            g.name.to_lowercase().contains(&q)
        })
        .map(|(idx, _)| idx)
        .collect()
}

pub(crate) fn active_results_field_mut(state: &mut ResultsState) -> &mut String {
    match state.cursor {
        0 => &mut state.resolution,
        1 => &mut state.preset,
        2 => &mut state.avg_fps,
        3 => &mut state.fps_1_low,
        4 => &mut state.fps_01_low,
        _ => &mut state.upscaling,
    }
}
