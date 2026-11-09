# rtop

**A fast, elegant terminal system monitor in Rust — CPU, memory, disks, network, GPU, and processes on Linux, macOS, and Windows.**

English | [简体中文](README.zh-CN.md)

`rtop` is a native Rust take on the btop experience. It samples the system
through a small, typed snapshot layer, gives every view a calm gradient-driven
design, and keeps terminal input and output defensive by default. It is a
single binary with no runtime services and no shell commands.

## Contents

- [Features](#features)
- [Screenshot](#screenshot)
- [Platform support](#platform-support)
- [Installation](#installation)
- [Usage](#usage)
- [Interactive keys](#interactive-keys)
- [Configuration](#configuration)
- [Themes](#themes)
- [Security and robustness](#security-and-robustness)
- [How it works](#how-it-works)
- [Development](#development)
- [Project layout](#project-layout)
- [Limitations](#limitations)
- [License](#license)

## Features

- Responsive overview plus dedicated **CPU**, **memory + disks**, **network**,
  **process**, and **GPU** views.
- Braille, block, and plain-ASCII graphs with three-stop gradients and
  per-core history.
- CPU package and per-core usage, model, frequency, load average, uptime,
  temperature, and optional battery status.
- RAM, swap, cached memory (Linux), free/available breakdown, mount usage, and
  disk I/O rates.
- Network download/upload histories, automatic busiest-interface selection,
  link speed, addresses, and totals.
- Process table with CPU/memory/PID/name/user sorting, reverse order, filtering,
  tree view, details, and signal delivery. `←`/`→` cycles sort fields quickly,
  and Linux tasks (threads) are never duplicated as separate processes.
- Best-effort GPU monitoring: utilisation, VRAM, temperature, power, clocks,
  fans, and history. NVIDIA uses NVML on Linux and Windows; AMD/Intel use the
  Linux DRM/hwmon sysfs interfaces.
- Eight built-in themes (including the new **Sakura** palette) and btop-compatible user `.theme` files. Dotted memory gauges are the default and can be switched back to solid blocks with `mem_style = "blocks"`.
- Full mouse support: clickable tabs, process rows, interface rows, and wheel
  scrolling.
- `--dump` renders one frame without a TTY, which is useful in CI, scripts, and
  screenshots.
- No shell execution, no network access, bounded file reads, ANSI/bidi
  sanitisation, signal-safe terminal restoration, and PID-reuse checks before
  signalling.

## Screenshot

```text
  rtop   0 all   1 cpu   2 mem   3 net   4 proc                                      ? 14:32:08
 mybox · you · Linux (Fedora 42)  AMD Ryzen 7 7700   up 03:42:10 · load 1.24 0.98 0.74 · 3.80 GHz
╭─ cpu 27%  8 cores──────────────────────────────╮╭─ mem 61%  9.8 GiB / 16.0 GiB────────────────╮
│27%  avg 22%  peak 64%              100% C0 ⣿⣿⣿⣿⣤⣀   41% ││Used      ●●●●●●●●●●●●●●·······   9.8 GiB  61%│
│                                        C1 ⣿⣦⣀⣀⣀⣀   18% ││Available ●●●●●●●●···········   6.2 GiB  39%│
│                                    50% C2 ⣿⣿⣶⣀⣀⣀   33% ││Cached    ●●●●●··············   2.1 GiB  13%│
│                                        C3 ⣤⣀⣀⣀⣀⣀    9% ││Free      ●●●●●●●············   4.1 GiB  26%│
│                                        C4 ⣶⣤⣀⣀⣀⣀   24% ││Swap      ●●●················   1.0/8.0G  13%│
│⣀⣀⣀⣀⣀⣀⣀⣀⣀⣀⣀⣀⣀⣀⣀⣀⣀⣀⣀⣀⣀⣀⣀⣀⣀⣀⣀⣀⣀⠉⠉⠉⠉⠒⠒⠒⠒⠤⠤⣀⣀⣀⣀⠉⠉⠒⠒⠤⣀     0% ││disk                     R 18.4 MiB/s  W 2.1 MiB/s│
│usr 12%  sys 9%  wait 1%  idle 78%              ││/         ●●●●●●●●●●●●●●●●●······  58%     412G/710G│
╰────────────────────────────────────────────────╯╰─────────────────────────────────────────────────╯
╭─ net wlan0  ↓ 24.8 MiB/s  ↑ 1.2 MiB/s──────────────────────────────────────────────────────────────────────────────╮
│↓ download 24.8 MiB/s (208 Mbit/s)                                                              total 84.2 GiB     │
│24.8 MiB/s                                                                                                    64M/s │
│⣀⣀⣀⣀⣀⣀⣀⣀⣀⣀⣀⣀⣀⣀⣀⣀⣀⣀⣀⣀⣀⣀⣀⣀⣀⣀⣀⣀⣀⣀⣀⣀⣀⣀⣀⣀⣀⣀⣀⣀⣀⣀⣀⣀⣀⣀⣀⣀⣀⣀⣀⣀⣀⣀⣀⣀⣀⣀⣀⣀⣀⣀⣀⣀⣀⣀⣀⣀⣀⣀⣀⣀⣀⣀⣀⣀⣀⣀⣀⣀⣀⣀⣀⣀⣀⣀⣀⣀⣀⣀⣀⣀⣀⣀⣀⣀⣀⣀⣀⣀⣀⣀⣀⣀⣀⣀⣀⣀⣀     0 │
│↑ upload 1.2 MiB/s (9.6 Mbit/s)                                                                  total 8.4 GiB      │
╰──────────────────────────────────────────────────────────────────────────────────────────────────────────────────────╯
╭─ proc 186/186  cpu ↓  12 run─────────────────────────────────────────────────────────────────────────────────────────╮
│ R    812 firefox            /usr/lib/firefox/firefox                                     you          1.4G ██▍  12.4 │
│ S   1021 terminal           /usr/bin/alacritty                                           you          184M ██    5.8 │
│ S   1044 code               /usr/share/code/code --type=renderer                        you          912M ██    4.1 │
│ S      1 systemd            /usr/lib/systemd/systemd --switched-root --system            root         14M ░     0.0 │
╰──────────────────────────────────────────────────────────────────────────────────────────────────────────────────────╯
 ↑↓ move  ←→ sort  Enter info  f filter  r reverse  t tree  k kill  ? help  q quit
```

The exact layout adapts to the terminal size. Narrow terminals switch to a
compact view automatically; press `1`–`5` to see one view at a time.

When a supported GPU is present, `5` opens a dedicated GPU view and the
overview adds a compact GPU column on wide terminals:

```text
  rtop  │  0 all   1 cpu   2 mem   3 net   4 proc   5 gpu                           ? keys 16:22:11
╭─ gpu0 68%  nvidia──────────────────────────────╮╭─ gpu1 12%  amd─────────────────────────────────╮
│NVIDIA GeForce RTX 4090                   nvidia││AMD Radeon 780M                     amd · amdgpu│
│util     ██████████████████████▌░░░░░░░░░░   68%││util     ███▉░░░░░░░░░░░░░░░░░░░░░░░░░░░░░   12%│
│vram      ███████████████▊░░░░░    18G/24G   75%││vram      ██████████▌░░░░░░░░░░  2.0G/4.0G   50%│
│72°C · 315 W · core 2745 MHz · mem 10501 MHz    ││48°C · 22 W · core 1800 MHz                     │
│history                                 peak 68%││history                                 peak 12%│
│                                                ││                                                │
│                                               ⢸││                                                │
│                                               ⢸││                                                │
│⣀⣀⣀⣀⣀⣀⣀⣀⣀⣀⣀⣀⣀⣀⣀⣀⣀⣀⣀⣀⣀⣀⣀⣀⣀⣀⣀⣀⣀⣀⣀⣀⣀⣀⣀⣀⣀⣀⣀⣀⣀⣀⣀⣀⣀⣀⣀⢸││⣀⣀⣀⣀⣀⣀⣀⣀⣀⣀⣀⣀⣀⣀⣀⣀⣀⣀⣀⣀⣀⣀⣀⣀⣀⣀⣀⣀⣀⣀⣀⣀⣀⣀⣀⣀⣀⣀⣀⣀⣀⣀⣀⣀⣀⣀⣀⢸│
╰────────────────────────────────────────────────╯╰────────────────────────────────────────────────╯
 Tab next  +/- speed  s pause  ? help  q quit
```

## Platform support

| Capability | Linux | macOS | Windows |
| --- | :---: | :---: | :---: |
| CPU, cores, frequency, load, uptime | Yes | Yes | Yes |
| Memory and swap | Yes | Yes | Yes |
| Disk usage and I/O | Yes | Yes | Yes |
| Network rates and totals | Yes | Yes | Yes |
| Processes, sorting, filtering, tree | Yes | Yes | Yes |
| Process thread counts | Yes | Not exposed | Not exposed |
| Process signals | Unix signals | Unix signals | TerminateProcess |
| CPU temperatures | sysinfo sensors | sysinfo sensors | sysinfo sensors* |
| Battery indicator | sysfs | Not yet | `GetSystemPowerStatus` |
| NVIDIA GPU | NVML (usage/VRAM/temp/power/clocks) | Not yet | NVML |
| AMD / Intel GPU | sysfs (usage/VRAM/temp/power/clocks) | Not yet | Not yet |

\* Temperature availability depends on the firmware/drivers exposing sensors to
the operating system. Missing sensors simply hide the indicator.

The UI uses Unicode braille and box drawing by default. If a terminal or font
cannot render them, run with `--tty` for pure-ASCII borders, bars, arrows, and
`#` graphs.

## Installation

Requirements: Rust 1.85 or newer, and a UTF-8 capable terminal. Windows
Terminal, iTerm2, WezTerm, Alacritty, Kitty, GNOME Terminal, Konsole, and
similar terminals are supported.

Build and install from this repository:

```bash
cargo install --path .
rtop
```

Or install directly from Git:

```bash
cargo install --git https://github.com/summertools/rtop.git
```

For local development:

```bash
cargo build --release
./target/release/rtop
```

Release archives contain a single `rtop` executable (`rtop.exe` on
Windows). The release workflow publishes `rtop-linux-x86_64.tar.gz`,
`rtop-macos-aarch64.tar.gz`, `rtop-macos-x86_64.tar.gz`, and
`rtop-windows-x86_64.zip`, plus a `SHA256SUMS` file for integrity checks.

## Usage

```text
rtop [OPTIONS]
```

| Option | Description |
| --- | --- |
| `-p, --preset <0-5>` | Start at the overview, CPU, memory, network, process, or GPU view |
| `-u, --update <MS>` | Refresh interval, `250`–`10000` ms (default `1000`) |
| `--theme <NAME>` | Built-in theme or a theme file in the user theme directory |
| `--graph-symbol <STYLE>` | `braille`, `block`, or `tty` |
| `-t, --tty` | Force 16-colour, ASCII-friendly output |
| `--low-color` | Quantise colours to the 256-colour palette |
| `--no-color` | Disable colour entirely (selection uses reverse video) |
| `--no-mouse` | Do not capture the mouse |
| `--read-only` | Disable process signalling |
| `--config <FILE>` | Use an explicit configuration file |
| `--dump` | Render one plain frame to stdout and exit |
| `--width <COLS>` | Width used by `--dump` (default `120`) |
| `--height <ROWS>` | Height used by `--dump` (default `36`) |
| `--list-themes` | List built-in and user themes |
| `-h, --help` / `-V, --version` | Help and version |

`--dump` does not require a terminal. It is the easiest way to check a build on
a server or in CI:

```bash
rtop --dump --width 100 --height 30 --preset 1
```

### Environment variables

| Variable | Effect |
| --- | --- |
| `NO_COLOR` | Disables colour (same as `--no-color`) |
| `CLICOLOR` | `CLICOLOR=0` disables colour |
| `COLORTERM` | `truecolor` or `24bit` enables 24-bit colour |
| `TERM` | `256color` selects the 256-colour palette; `dumb` is rejected in interactive mode |
| `TERM_PROGRAM`, `WT_SESSION` | Helps detect true-colour terminals |
| `USER`, `USERNAME` | Used for the header identity fallback |

## Interactive keys

| Key | Action |
| --- | --- |
| `↑`, `↓`, `j` | Move the process selection |
| `PgUp`, `PgDn` | Move one page |
| `Home`, `End`, `g`, `G` | First / last process |
| `Enter`, `i` | Process details |
| `f` | Filter processes (type, `Enter` applies, `Esc` cancels, `Ctrl+U` clears) |
| `←`, `→` | Previous / next process sort field |
| `c`, `m`, `p`, `n`, `u` | Sort by CPU, memory, PID, name, or user |
| `r` | Reverse the sort order |
| `t` | Toggle tree view |
| `e` / `w` / `Space` | Expand / collapse a process subtree |
| `k` | Send a signal (confirmation dialog) |
| `0`–`5`, `Tab`, `Shift+Tab` | Switch views |
| `[`, `]` | Previous / next network interface |
| `a` | Toggle automatic interface selection |
| `+`, `-` | Faster / slower refresh |
| `s` | Pause or resume updates |
| `F1`, `?` | Help |
| `F2` | Cycle built-in themes |
| `x` | Clear the process filter |
| `Esc` | Close an overlay, clear a filter, or return to the overview |
| `q`, `Ctrl+C` | Quit |

Mouse clicks work on the view tabs, process rows (double-click opens details),
network interface rows, and the process list scroll wheel.

## Configuration

The configuration file is optional; sensible defaults are used when it is
absent. Locations:

| Platform | Path |
| --- | --- |
| Linux | `$XDG_CONFIG_HOME/rtop/rtop.toml`, otherwise `~/.config/rtop/rtop.toml` |
| macOS | `~/Library/Application Support/rtop/rtop.toml` |
| Windows | `%APPDATA%\rtop\rtop.toml` |

Example:

```toml
# Refresh interval in milliseconds (250-10000)
update_ms = 1000

# 0 overview, 1 CPU, 2 memory/disks, 3 network, 4 processes, 5 GPUs
preset = 0

# braille, block, or tty
graph_symbol = "braille"

# Built-in theme or a .theme file in the themes directory
theme = "default"

truecolor = true
tty_mode = false
rounded_corners = true
theme_background = false
show_battery = true
show_gpu = true         # GPU view and summary when a device is detected
mem_style = "dots"      # dots | blocks (memory gauges)
proc_sort = "cpu"       # cpu, memory, pid, name, user
proc_tree = false
proc_reversed = false

net_auto = true
net_iface = ""          # preferred interface when net_auto = false
mouse = true
read_only = false
show_key_hints = true
```

Invalid values are clamped or ignored with a warning; a malformed file never
prevents the monitor from starting. Unknown keys are ignored for forward
compatibility. Configuration and theme files are limited to 128 KiB.

## Themes

Built-in themes: `default` (Tokyo Night), `dracula`, `gruvbox`, `nord`,
`catppuccin`, `sakura`, `light`, and `tty`. **Sakura** is a deep plum palette
with rose, mint, and lavender accents; it pairs particularly well with the
default dotted memory gauges.

User themes live in the platform configuration directory under `rtop/themes/`
(for example `~/.config/rtop/themes/` on Linux,
`~/Library/Application Support/rtop/themes/` on macOS, or
`%APPDATA%\rtop\themes\` on Windows) and use btop's familiar syntax:

```text
theme[main_bg]="#0b0e14"
theme[main_fg]="#c0caf5"
theme[title]="#7aa2f7"
theme[hi_fg]="#7dcfff"
theme[selected_bg]="#283457"
theme[selected_fg]="#ffffff"
theme[cpu_start]="#2ac3de"
theme[cpu_mid]="#7aa2f7"
theme[cpu_end]="#bb9af7"
```

Theme names are restricted to ASCII letters, digits, `-`, and `_`; path
traversal is rejected. Unknown keys and malformed colours are ignored. Existing
btop themes can be copied into the theme directory and selected with
`--theme mytheme`.

## Security and robustness

Terminal monitors read a lot of data controlled by other processes. `rtop`
treats that data as untrusted:

- **Terminal injection:** process names, command lines, users, mount points,
  interface names, and addresses pass through an ANSI/control/bidi sanitiser
  before drawing. ESC, C0/C1 controls, bidirectional overrides, and zero-width
  direction marks cannot alter the layout.
- **No shell:** the program never runs shell commands. Process signals are sent
  through the operating system API. GPU data comes from bounded sysfs reads and
  the locally installed NVML library; rtop never executes `nvidia-smi`.
- **PID reuse:** before sending a signal, the process start time and name are
  re-read and compared with the snapshot that the confirmation dialog showed.
  PID 1 (Unix), low system PIDs (Windows), and the monitor itself are refused.
  `--read-only` removes the dialog entirely.
- **Bounded reads:** configuration and theme files are capped at 128 KiB and
  theme names are whitelisted. Linux sysfs reads are size-capped and interface
  names are validated before path construction.
- **Terminal restoration:** raw mode, the alternate screen, mouse capture, and
  bracketed paste are restored by RAII, a panic hook, and `SIGTERM`/`SIGHUP`/
  `SIGQUIT` handlers.
- **Minimal unsafe:** rtop itself uses `unsafe` only for three small platform
  calls (`localtime_r` on Unix, `GetLocalTime` and `GetSystemPowerStatus` on
  Windows). GPU vendor data uses the safe `nvml-wrapper` library, which loads the
  driver dynamically and contains its own audited FFI.
- **No network access** and no background services. All data is read from the
  operating system and discarded when the process exits.

Process signalling is a deliberate, confirmed action and is subject to operating
system permissions. Run `--read-only` on shared or untrusted machines if you
want a strictly observational monitor.

## How it works

```text
┌──────────────┐   Snapshot   ┌──────────────┐   Canvas   ┌──────────────┐
│  Collector   │ ───────────▶ │     App      │ ─────────▶ │   Renderer   │
│ (OS sampling)│   channel    │ state/input  │   diff     │ (crossterm)  │
└──────────────┘              └──────────────┘            └──────────────┘
```

- A background collector samples the operating system at the configured
  interval and sends owned `Snapshot` values through a bounded channel. The UI
  thread never blocks on `/proc`, WMI, or IOKit.
- Platform data is normalised into plain Rust structs (`CpuSnapshot`,
  `MemorySnapshot`, `DiskSnapshot`, `NetworkSnapshot`, `GpuSnapshot`,
  `ProcessInfo`). Linux adds CPU time breakdowns, cached memory, interface
  speed/wireless state, GPU sysfs data, and battery information.
- The UI draws into a clipped cell canvas. Wide characters are handled
  explicitly, boxes and graphs are bounds-checked, and the previous frame is
  diffed so only changed cells reach the terminal.
- Histories live in the app, not the collector, so pausing updates freezes the
  view rather than discarding data.

## Development

```bash
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo test
cargo build --release
```

Cross-platform checks used by CI:

```bash
cargo check --target aarch64-apple-darwin
cargo check --target x86_64-pc-windows-gnu
```

The test suite covers formatting and width handling, sanitisation, theme
parsing, configuration validation, graph scaling, canvas wide-character repair,
process sorting/tree building/filtering, selection behaviour, signal safety, all
views at 40×12 through 200×60, overlays, and end-to-end CLI behaviour.

## Project layout

| Path | Purpose |
| --- | --- |
| `src/main.rs` | Binary entry point, collector thread, signals, event loop |
| `src/app.rs` | Application state, process listing, actions, input mapping |
| `src/cli.rs` | Command-line interface and overrides |
| `src/config.rs` | Configuration loading, validation, theme discovery |
| `src/format.rs` | Byte/rate/duration formatting, width-aware truncation, sanitisation |
| `src/platform/` | Cross-platform snapshot collector + Linux/Windows extras |
| `src/theme.rs` | Colour model, built-in themes, btop theme parser |
| `src/term.rs` | Raw terminal lifecycle, diff renderer, colour quantisation |
| `src/ui/` | Canvas, graphs, widgets, panels, overlays, responsive layout |
| `tests/cli.rs` | End-to-end CLI tests |
| `tests/render.rs` | Layout, overflow, and injection smoke tests |

## Limitations

- GPU monitoring is read-only and best effort. NVIDIA requires a working NVML
  driver; AMD/Intel support depends on the kernel exposing DRM/hwmon files.
  Apple GPUs, AMD/Intel GPUs on Windows, overclocking, and fan control are not
  implemented.
- Battery status is available on Linux (sysfs) and Windows
  (`GetSystemPowerStatus`), but not yet on macOS.
- The portable process backend does not expose a thread count on macOS or
  Windows; the details overlay shows `-` for it.
- Temperature sensors depend on what the operating system exposes. Containers
  and virtual machines often expose none.
- Metrics are snapshots at the refresh interval, not a hard real-time stream.
  Very large process tables can make a single sample slower; the collector runs
  off the UI thread so input remains responsive.
- Disk lists come from the platform's mounted filesystems. Bind mounts and
  pseudo filesystems are filtered or deduplicated by device and capacity, but
  unusual storage stacks may still produce surprising mount names.
- The interactive UI requires a terminal. Use `--dump` for scripts.

## License

MIT. See [LICENSE](LICENSE).

The design is inspired by [btop++](https://github.com/aristocratos/btop); this
project is an independent Rust implementation and shares no source code with it.
