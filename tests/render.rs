//! Layout and safety smoke tests: every view must render at every supported
//! size without panicking, overflow, or leaking control characters.

use std::sync::Arc;
use std::sync::atomic::AtomicU64;

use rtop::app::{App, Overlay, Preset, ToastLevel};
use rtop::config::{Config, GraphSymbol, MemStyle};
use rtop::format;
use rtop::platform::{
    BatteryInfo, BatteryState, CpuBreakdown, CpuSnapshot, DiskSnapshot, GpuSnapshot, GpuVendor,
    HostInfo, MemorySnapshot, NetworkSnapshot, ProcessInfo, ProcessState, ProcessSummary,
    SensorReading, Snapshot,
};
use rtop::theme::Theme;
use rtop::ui;

fn process(pid: u32, parent: Option<u32>, name: &str, cpu: f32, memory: u64) -> ProcessInfo {
    ProcessInfo {
        pid,
        parent,
        name: name.to_owned(),
        command: format!("{name} --with-arguments /tmp/日本語"),
        user: if pid % 3 == 0 { "root" } else { "tester" }.to_owned(),
        state: match pid % 5 {
            0 => ProcessState::Running,
            1 => ProcessState::Sleeping,
            2 => ProcessState::DiskSleep,
            3 => ProcessState::Zombie,
            _ => ProcessState::Idle,
        },
        cpu,
        memory,
        virtual_memory: memory.saturating_mul(2),
        threads: Some(4),
        start_time: 1_700_000_000,
        run_time: 3_721,
        exe: Some("/usr/bin/example".to_owned()),
        cwd: Some("/home/tester/projects/日本語".to_owned()),
        read_bytes: 1024,
        written_bytes: 2048,
        session: Some(1),
    }
}

fn snapshot() -> Snapshot {
    let mut processes = Vec::new();
    processes.push(process(1, None, "init", 0.1, 8 * 1024 * 1024));
    for index in 2..60u32 {
        let parent = if index < 20 {
            Some(1)
        } else {
            Some(2 + (index % 10))
        };
        processes.push(process(
            index,
            parent,
            &format!("worker-{index}\u{1b}[31m\nspoof"),
            (index % 17) as f32 * 0.7,
            u64::from(index) * 1_000_000,
        ));
    }
    Snapshot {
        host: HostInfo {
            hostname: "testbox".to_owned(),
            os: "TestOS 1.0".to_owned(),
            kernel: "6.0.0".to_owned(),
            arch: "x86_64".to_owned(),
            user: "tester".to_owned(),
            cpu_model: "Example CPU @ 3.20GHz".to_owned(),
        },
        cpu: CpuSnapshot {
            overall_usage: 37.5,
            cores: vec![10.0, 20.0, 30.0, 40.0, 50.0, 60.0, 70.0, 80.0],
            frequencies_mhz: vec![3200; 8],
            average_frequency_mhz: 3200,
            load: [1.25, 0.75, 0.5],
            uptime_secs: 123_456,
            breakdown: Some(CpuBreakdown {
                user: 25.0,
                system: 10.0,
                wait: 2.5,
                idle: 62.5,
            }),
            temperature: Some(61.2),
        },
        memory: MemorySnapshot {
            total: 16 * 1024 * 1024 * 1024,
            used: 6 * 1024 * 1024 * 1024,
            available: 10 * 1024 * 1024 * 1024,
            free: 8 * 1024 * 1024 * 1024,
            cached: Some(2 * 1024 * 1024 * 1024),
            swap_total: 4 * 1024 * 1024 * 1024,
            swap_used: 512 * 1024 * 1024,
            swap_free: 3_584 * 1024 * 1024,
        },
        disks: vec![
            DiskSnapshot {
                name: "/dev/sda1".to_owned(),
                mount: "/".to_owned(),
                filesystem: "ext4".to_owned(),
                total: 512 * 1024 * 1024 * 1024,
                available: 300 * 1024 * 1024 * 1024,
                used: 212 * 1024 * 1024 * 1024,
                read_rate: 12.5 * 1024.0 * 1024.0,
                write_rate: 3.0 * 1024.0 * 1024.0,
                removable: false,
                read_only: false,
            },
            DiskSnapshot {
                name: "/dev/sdb1".to_owned(),
                mount: "/mnt/data".to_owned(),
                filesystem: "xfs".to_owned(),
                total: 2 * 1024 * 1024 * 1024 * 1024,
                available: 1024 * 1024 * 1024 * 1024,
                used: 1024 * 1024 * 1024 * 1024,
                read_rate: 0.0,
                write_rate: 0.0,
                removable: true,
                read_only: false,
            },
        ],
        networks: vec![
            NetworkSnapshot {
                name: "eth0".to_owned(),
                received_rate: 1.5 * 1024.0 * 1024.0,
                transmitted_rate: 120.0 * 1024.0,
                total_received: 5 * 1024 * 1024 * 1024,
                total_transmitted: 800 * 1024 * 1024,
                speed_mbps: Some(1000),
                wireless: false,
                loopback: false,
                up: Some(true),
                addresses: vec!["192.0.2.10/24".to_owned(), "2001:db8::10/64".to_owned()],
                mac: "02:00:00:00:00:01".to_owned(),
            },
            NetworkSnapshot {
                name: "lo".to_owned(),
                received_rate: 1024.0,
                transmitted_rate: 1024.0,
                total_received: 0,
                total_transmitted: 0,
                speed_mbps: None,
                wireless: false,
                loopback: true,
                up: Some(true),
                addresses: vec!["127.0.0.1/8".to_owned()],
                mac: String::new(),
            },
        ],
        processes,
        process_summary: ProcessSummary {
            total: 60,
            running: 12,
            sleeping: 40,
            zombie: 2,
            threads: 180,
        },
        gpus: vec![
            GpuSnapshot {
                index: 0,
                name: "NVIDIA GeForce RTX 4070\u{1b}[31m".to_owned(),
                vendor: GpuVendor::Nvidia,
                driver: "nvidia".to_owned(),
                pci: Some("01:00.0".to_owned()),
                utilization: Some(42.0),
                memory_used: Some(6 * 1024 * 1024 * 1024),
                memory_total: Some(12 * 1024 * 1024 * 1024),
                temperature: Some(58.0),
                power_w: Some(112.0),
                core_clock_mhz: Some(2_475),
                memory_clock_mhz: Some(10_500),
                fan_percent: Some(38.0),
            },
            GpuSnapshot {
                index: 1,
                name: "AMD Radeon 780M".to_owned(),
                vendor: GpuVendor::Amd,
                driver: "amdgpu".to_owned(),
                pci: Some("0a:00.0".to_owned()),
                utilization: Some(7.0),
                memory_used: Some(512 * 1024 * 1024),
                memory_total: Some(2 * 1024 * 1024 * 1024),
                temperature: Some(45.0),
                power_w: Some(9.0),
                core_clock_mhz: Some(1_100),
                memory_clock_mhz: None,
                fan_percent: None,
            },
        ],
        sensors: vec![SensorReading {
            label: "Package id 0".to_owned(),
            temperature: 61.2,
            max: Some(80.0),
            critical: Some(100.0),
        }],
        battery: Some(BatteryInfo {
            percent: 87,
            state: BatteryState::Discharging,
            minutes_remaining: Some(123),
            power_w: Some(14.2),
        }),
        warnings: Vec::new(),
    }
}

fn app(config: Config) -> App {
    let theme = Theme::builtin(&config.theme).unwrap_or_else(|| Theme::builtin("default").unwrap());
    let mut app = App::new(config, theme, Vec::new(), Arc::new(AtomicU64::new(1000)));
    app.apply_snapshot(snapshot());
    app
}

#[test]
fn all_presets_render_without_overflow() {
    for graph in [GraphSymbol::Braille, GraphSymbol::Block, GraphSymbol::Tty] {
        for theme in Theme::builtin_names() {
            let config = Config {
                graph_symbol: graph,
                theme: (*theme).to_owned(),
                ..Config::default()
            };
            let app = app(config);
            for preset in Preset::all() {
                for (width, height) in [
                    (40, 12),
                    (60, 20),
                    (80, 24),
                    (100, 30),
                    (120, 36),
                    (200, 60),
                ] {
                    let mut view = app.clone();
                    view.preset = preset;
                    let (canvas, _) = ui::render(&view, width, height);
                    assert_eq!(canvas.width(), width);
                    assert_eq!(canvas.height(), height);
                    for line in canvas.plain().lines() {
                        assert!(
                            format::width(line) <= usize::from(width),
                            "line wider than {width}: {line:?}"
                        );
                        assert!(!line.contains('\u{1b}'), "escape leaked: {line:?}");
                        assert!(!line.contains('\u{202e}'), "bidi leaked: {line:?}");
                        if graph == GraphSymbol::Tty {
                            assert!(line.is_ascii(), "TTY output is not ASCII: {line:?}");
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn memory_gauges_support_dotted_and_block_styles() {
    let mut dotted = app(Config {
        preset: 2,
        mem_style: MemStyle::Dots,
        ..Config::default()
    });
    dotted.preset = Preset::Memory;
    let (canvas, _) = ui::render(&dotted, 120, 36);
    let used = canvas
        .plain()
        .lines()
        .find(|line| line.contains("Used"))
        .expect("Used row")
        .to_owned();
    assert!(used.contains('●'), "dotted Used row: {used}");
    assert!(!used.contains('█'), "dotted row contains blocks: {used}");

    let mut blocks = dotted.clone();
    blocks.config.mem_style = MemStyle::Blocks;
    let (canvas, _) = ui::render(&blocks, 120, 36);
    let used = canvas
        .plain()
        .lines()
        .find(|line| line.contains("Used"))
        .expect("Used row")
        .to_owned();
    assert!(used.contains('█'), "block Used row: {used}");
    assert!(!used.contains('●'), "block row contains dots: {used}");
}

#[test]
fn gpu_view_renders_devices_and_absent_hint() {
    let config = Config {
        preset: 5,
        ..Config::default()
    };
    let app = app(config);
    let (canvas, _) = ui::render(&app, 160, 46);
    let plain = canvas.plain();
    assert!(plain.contains("gpu0"), "missing GPU card: {plain}");
    assert!(plain.contains("NVIDIA GeForce RTX 4070"));
    assert!(plain.contains("vram"));
    assert!(plain.contains("history"));

    let mut one = app.clone();
    one.preset = Preset::Gpu;
    if let Some(snapshot) = one.snapshot.as_mut() {
        snapshot.gpus.truncate(1);
    }
    for (width, height) in [(40, 12), (80, 24), (120, 36), (200, 60)] {
        let (canvas, _) = ui::render(&one, width, height);
        for line in canvas.plain().lines() {
            assert!(format::width(line) <= usize::from(width));
        }
    }

    let mut empty = app.clone();
    if let Some(snapshot) = empty.snapshot.as_mut() {
        snapshot.gpus.clear();
    }
    let (canvas, _) = ui::render(&empty, 120, 36);
    assert!(canvas.plain().contains("no supported GPU"));
}

#[test]
fn gpu_summary_appears_in_the_header_when_devices_exist() {
    let app = app(Config::default());
    let (canvas, _) = ui::render(&app, 160, 46);
    assert!(canvas.plain().contains("GPU 42% ×2"));
}

#[test]
fn hiding_gpu_removes_the_tab_and_summary() {
    let config = Config {
        show_gpu: false,
        ..Config::default()
    };
    let app = app(config);
    let (canvas, _) = ui::render(&app, 160, 46);
    let plain = canvas.plain();
    assert!(!plain.contains("5 gpu"));
    assert!(!plain.contains("GPU 42%"));
}

#[test]
fn overlays_render_at_small_and_large_sizes() {
    let mut app = app(Config::default());
    for (width, height) in [(40, 12), (80, 24), (120, 36)] {
        app.overlay = Overlay::Help { scroll: 4 };
        let (canvas, _) = ui::render(&app, width, height);
        assert!(canvas.plain().contains("HELP") || canvas.plain().contains("help"));
        app.overlay = Overlay::ProcessInfo { pid: 42, scroll: 0 };
        let (canvas, _) = ui::render(&app, width, height);
        assert!(canvas.plain().contains("process 42"));
        app.overlay = Overlay::ConfirmKill {
            pid: 42,
            name: "worker-42".to_owned(),
            start_time: 1_700_000_000,
            signal: 0,
        };
        let (canvas, _) = ui::render(&app, width, height);
        assert!(canvas.plain().contains("send signal"));
    }
}

#[test]
fn tiny_terminals_show_a_hint_instead_of_panicking() {
    for (width, height) in [(0, 0), (10, 3), (20, 8), (39, 11)] {
        let app = app(Config::default());
        let (canvas, _) = ui::render(&app, width, height);
        assert_eq!(canvas.width(), width);
        assert_eq!(canvas.height(), height);
        if width >= 20 && height >= 8 {
            assert!(canvas.plain().contains("too small"));
        }
    }
}

#[test]
fn no_snapshot_and_paused_states_render() {
    if let Some(mut app) = Some(app(Config::default())) {
        app.paused = true;
        let (canvas, _) = ui::render(&app, 100, 30);
        assert!(canvas.plain().contains("paused") || canvas.plain().contains("PAUSED"));
    }
    let theme = Theme::builtin("default").unwrap();
    let app = App::new(
        Config::default(),
        theme,
        vec!["a warning".to_owned()],
        Arc::new(AtomicU64::new(1000)),
    );
    let (canvas, _) = ui::render(&app, 100, 30);
    assert!(canvas.plain().contains("collecting"));
}

#[test]
fn layout_grid_never_overflows() {
    let app = app(Config::default());
    let widths = [
        20, 39, 40, 41, 59, 60, 71, 72, 73, 79, 80, 83, 84, 85, 99, 100, 101, 119, 120, 121, 150,
        200, 400,
    ];
    let heights = [
        8, 11, 12, 13, 16, 17, 18, 19, 20, 21, 23, 24, 25, 27, 30, 36, 50,
    ];
    for preset in Preset::all() {
        let mut view = app.clone();
        view.preset = preset;
        for width in widths {
            for height in heights {
                let (canvas, _) = ui::render(&view, width, height);
                for line in canvas.plain().lines() {
                    assert!(
                        format::width(line) <= usize::from(width.min(ui::MAX_CANVAS_WIDTH)),
                        "overflow at {width}x{height} preset {preset:?}: {line:?}"
                    );
                }
            }
        }
    }
}

#[test]
fn extreme_terminal_sizes_are_capped() {
    let app = app(Config::default());
    let (canvas, _) = ui::render(&app, u16::MAX, u16::MAX);
    assert_eq!(canvas.width(), ui::MAX_CANVAS_WIDTH);
    assert_eq!(canvas.height(), ui::MAX_CANVAS_HEIGHT);
}

#[test]
fn selected_process_row_keeps_its_background_behind_text() {
    let config = Config {
        preset: 4,
        ..Config::default()
    };
    let app = app(config);
    let (canvas, layout) = ui::render(&app, 120, 36);
    let index = app.selected_row().expect("selected row");
    let y = layout.proc_body.y + u16::try_from(index - app.proc_scroll).unwrap();
    let cell = canvas.cell(layout.proc_body.x + 12, y);
    assert_eq!(
        cell.style.bg,
        app.theme.highlight_bg.into(),
        "selection background was painted over"
    );
}

#[test]
fn toast_levels_are_rendered() {
    let mut app = app(Config::default());
    for level in [ToastLevel::Info, ToastLevel::Warning, ToastLevel::Error] {
        app.set_toast("something happened", level);
        let (canvas, _) = ui::render(&app, 120, 36);
        assert!(canvas.plain().contains("something happened"));
    }
}
