# Changelog

All notable changes to this project are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and the project uses
[Semantic Versioning](https://semver.org/spec/v2.0.0.html).

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

[0.1.1]: https://github.com/summertools/rtop/releases/tag/v0.1.1
[0.1.0]: https://github.com/summertools/rtop/releases/tag/v0.1.0
