# Changelog

All notable changes to this project are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and the project uses
[Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.8] - 2026-11-15

### Changed

- Added thin, theme-aware section rules for the memory breakdown, history,
  disks, and top-memory processes. Disk capacity and I/O now read as a distinct
  section instead of flowing directly out of the RAM history graph.
- The full memory view groups Used/Available and Cached/Free into two-column
  cards, each with its exact value, percentage, and a restrained bar. Swap is
  kept as a separate full-width row. Compact overview tiles retain their
  one-line metrics so they still fit small panels.

[0.1.8]: https://github.com/summertools/rtop/releases/tag/v0.1.8

## [0.1.7] - 2026-11-15

### Fixed

- Restored the familiar full-width memory rows for Used, Available, Cached,
  Free, and Swap. Each row again shows its exact value, percentage, and meter;
  the 0.1.6 two-column grid felt cramped and lost the clear row-by-row reading
  order, especially on narrower terminals.
- Kept the used/cached/free composition strip, solid-block default, full-width
  history, and stacked disk/top-memory sections.

[0.1.7]: https://github.com/summertools/rtop/releases/tag/v0.1.7

## [0.1.6] - 2026-11-15

### Fixed

- Restored the detailed memory breakdown that 0.1.5 hid: Used, Available,
  Cached, Free, and Swap each show their exact value, percentage, and compact
  bar in a responsive two-column grid. Swap gets its own cell instead of a
  full-width row, keeping the layout balanced.

### Changed

- The breakdown uses full bars on wide panels, compact bars on medium panels,
  and a one-column text grid on very narrow terminals. Solid gauges remain the
  default; dotted gauges are still available through `mem_style = "dots"`.

[0.1.6]: https://github.com/summertools/rtop/releases/tag/v0.1.6

## [0.1.5] - 2026-11-15

### Changed

- Redesigned the memory panel around a single used/cached/free composition
  strip with an explanatory legend. Exact available memory and swap state now
  sit on a quiet fact line instead of five repeated full-width meters.
- The memory history graph now spans the panel, and disk/top-memory sections
  stack vertically so wide terminals can show per-disk I/O and process share
  bars without dead columns.
- Top-memory rows now include rank numbers and proportional bars. Disk rows
  expose per-disk read/write rates when width allows.
- Memory gauges now default to solid blocks. The dotted style remains available
  with `mem_style = "dots"`.

### Fixed

- The Linux thread-info test no longer fails when a short-lived test-harness
  thread disappears between reading `/proc/self/task` and
  `/proc/<tid>/status`.

[0.1.5]: https://github.com/summertools/rtop/releases/tag/v0.1.5

## [0.1.4] - 2026-11-15

### Added

- Linux ARM64 (aarch64) release archive: `rtop-linux-aarch64-musl.tar.gz`, a
  static build that runs on both glibc and musl distributions.
- CI now exercises Linux ARM64 natively and builds the static musl target used
  by releases.
- CPU and GPU model labels in the second header line. The active view gets
  priority (GPU in the GPU view, CPU elsewhere), and both are shown when the
  terminal is wide enough.
- GPU cards now include the PCI bus location (`pci 01:00.0`) when the driver
  exposes it and the sensor row has room.
- Blank GPU names fall back to a stable vendor/PCI/index description instead of
  leaving an empty card.

### Fixed

- Header model labels and right-aligned facts no longer compete for the same
  cells on narrow terminals.
- Hardware model and PCI strings are sanitised and length-bounded before
  layout, preventing control-character leaks or width distortion.

[0.1.4]: https://github.com/summertools/rtop/releases/tag/v0.1.4

## [0.1.3] - 2026-11-09

### Added

- New **Sakura** built-in theme: deep plum background with rose, mint, and
  lavender accents.
- Dotted memory gauges (`mem_style = "dots"`, the default): filled samples use
  round dots with a subtle dotted track, and cached memory uses a distinct
  ring-dot glyph. `mem_style = "blocks"` restores the previous solid bars for
  users who prefer them.

### Changed

- Memory composition, metric rows, and the disk section in the memory panel all
  follow the selected gauge style, including a pure-ASCII TTY fallback.
- README/docs screenshots and theme lists updated for the new palette and
  dotted memory treatment.

[0.1.3]: https://github.com/summertools/rtop/releases/tag/v0.1.3

## [0.1.2] - 2026-11-09

### Added

- Best-effort GPU monitoring with a dedicated GPU view and overview summary:
  utilisation, VRAM, temperature, power, core/memory clocks, fan speed, and
  utilisation history.
- NVIDIA GPUs are queried through NVML on Linux and Windows when the driver
  library is available; AMD and Intel GPUs are discovered through Linux DRM and
  hwmon sysfs files. GPU probing is read-only, bounded, and never invokes a
  shell command.
- `show_gpu` configuration option, `5` preset/`--preset 5`, and GPU history.

### Changed

- Process sorting now cycles fields with `←`/`→`, matching btop. Tree expansion
  remains on `e`/`w`/`Space`; the footer and help screen document the new keys.
- Refined the visual hierarchy: a lifted navigation band, accented active tabs,
  a subtler footer band, a memory composition strip, two-column core grids on
  wider layouts, colour-coded per-core/process CPU values, and a richer process
  header with running-count and filter status.

### Fixed

- Linux thread-group tasks exposed by sysinfo are no longer listed as separate
  processes, so one program can no longer appear as a pile of duplicate rows.
  The process list also de-duplicates by PID defensively.
- Process table selection, arrows, and sort indicators stay correct after
  filtering and duplicate removal.

[0.1.2]: https://github.com/summertools/rtop/releases/tag/v0.1.2

## [0.1.1] - 2026-11-07

### Fixed

- Hide the Windows System Idle Process (PID 0) from the process list. It is
  not a real signalable process and previously appeared in the table and
  process count on Windows.

## [0.1.0] - 2026-11-07

### Added

- Cross-platform sampling for Linux, macOS, and Windows through a typed
  snapshot layer built on `sysinfo`.
- Responsive overview and dedicated CPU, memory/disk, network, and process
  views.
- Braille, block, and ASCII/TTY graphs with gradient colouring and history.
- Per-core CPU usage, frequency, load average, uptime, sensors, and optional
  battery status.
- Memory, swap, cached memory (Linux), mount usage, and disk I/O rates.
- Network interface discovery, automatic interface selection, download and
  upload histories, link speed, addresses, and totals.
- Process table with CPU/memory sorting, filtering, tree mode, details,
  signals, and read-only mode.
- Built-in Tokyo Night, Dracula, Gruvbox, Nord, Catppuccin, Light, and TTY
  themes; user `.theme` files compatible with btop.
- Mouse support for tabs, process rows, scrolling, and interface selection.
- `--dump` mode for terminal-free rendering, tests, screenshots, and scripts.
- Defensive input handling: ANSI/bidi sanitisation, bounded configuration and
  theme reads, signal-safe terminal restoration, PID reuse verification before
  signalling, and no shell command execution.
- Unit, integration, layout, and CLI test suites with `cargo fmt` and strict
  `clippy` enforcement.

[Unreleased]: https://github.com/summertools/rtop/compare/v0.1.8...HEAD
[0.1.1]: https://github.com/summertools/rtop/releases/tag/v0.1.1
[0.1.0]: https://github.com/summertools/rtop/releases/tag/v0.1.0
