# Changelog

All notable changes to this project are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and the project uses
[Semantic Versioning](https://semver.org/spec/v2.0.0.html).

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

[0.1.0]: https://github.com/summertools/btop-rs/releases/tag/v0.1.0
