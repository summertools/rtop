# rtop

**用 Rust 编写的快速、优雅的终端系统监视器：在 Linux、macOS 和 Windows 上查看 CPU、内存、磁盘、网络、GPU 与进程。**

[English](README.md) | 简体中文

`rtop` 是对 btop 使用体验的原生 Rust 实现。它通过一层类型化的系统快照采样数据，
用渐变色彩和自适应布局呈现信息，并在终端输入输出上默认采取防御性策略。它只有一个二进制，
不需要后台服务，也不会调用 shell。

## 目录

- [功能](#功能)
- [界面截图](#界面截图)
- [平台支持](#平台支持)
- [安装](#安装)
- [使用](#使用)
- [交互按键](#交互按键)
- [配置文件](#配置文件)
- [主题](#主题)
- [安全与健壮性](#安全与健壮性)
- [工作原理](#工作原理)
- [开发](#开发)
- [项目结构](#项目结构)
- [已知限制](#已知限制)
- [许可证](#许可证)

## 功能

- 自适应总览，以及独立的 **CPU**、**内存 + 磁盘**、**网络**、**进程**、**GPU** 视图。
- Braille、方块和纯 ASCII 三种图形符号，三色渐变与历史曲线。
- CPU 总体/每核心占用、型号、频率、负载、运行时间、温度和电池状态。
- 内存、交换分区、缓存（Linux）、空闲/可用内存、挂载点占用和磁盘 I/O 速率。
- 网络下载/上传历史、自动选择最忙网卡、链路速率、IP/MAC 与累计流量。
- 进程表支持 CPU/内存/PID/名称/用户排序、反向排序、过滤、树形视图、详情与信号发送；
  `←`/`→` 快速切换排序字段，Linux 线程任务不会再被重复显示为多个进程。
- 尽力提供 GPU 监控：占用率、显存、温度、功耗、频率、风扇与历史曲线。Linux/Windows
  的 NVIDIA 设备通过 NVML 读取，Linux 的 AMD/Intel 设备通过 DRM/hwmon sysfs 读取。
- 7 个内置主题，并兼容 btop 的 `.theme` 用户主题文件。
- 完整鼠标支持：点击标签页、进程行、网卡行以及滚轮滚动。
- `--dump` 可在没有 TTY 的环境渲染一帧，适合 CI、脚本和截图。
- 不执行 shell、不访问网络、限制文件读取大小、转义 ANSI/双向控制字符、
  信号安全的终端恢复，以及发送信号前的 PID 复用校验。

## 界面截图

```text
  rtop   0 总览   1 cpu   2 内存   3 网络   4 进程                              ? 14:32:08
 mybox · you · Linux (Fedora 42)  AMD Ryzen 7 7700   up 03:42:10 · load 1.24 0.98 0.74 · 3.80 GHz
╭─ cpu 27%  8 cores──────────────────────────────╮╭─ mem 61%  9.8 GiB / 16.0 GiB────────────────╮
│27%  avg 22%  peak 64%              100% C0 ⣿⣿⣿⣿⣤⣀   41% ││Used      █████████████▎░░░░░░░   9.8 GiB  61%│
│                                        C1 ⣿⣦⣀⣀⣀⣀   18% ││Available ███████▊░░░░░░░░░░░   6.2 GiB  39%│
│                                    50% C2 ⣿⣿⣶⣀⣀⣀   33% ││Cached    ████▏░░░░░░░░░░░░░░   2.1 GiB  13%│
│                                        C3 ⣤⣀⣀⣀⣀⣀    9% ││Free      ██████▎░░░░░░░░░░░░   4.1 GiB  26%│
│⣀⣀⣀⣀⣀⣀⣀⣀⣀⣀⣀⣀⣀⣀⣀⣀⣀⣀⣀⣀⣀⣀⠉⠉⠒⠒⠤⣀⣀⣀⣀⠉⠒⠒⠤⣀     0% ││disk                     R 18.4 MiB/s  W 2.1 MiB/s│
│usr 12%  sys 9%  wait 1%  idle 78%              ││/         ████████████████▍░░░░  58%     412G/710G│
╰────────────────────────────────────────────────╯╰─────────────────────────────────────────────────╯
╭─ net wlan0  ↓ 24.8 MiB/s  ↑ 1.2 MiB/s──────────────────────────────────────────────────────────────────────────────╮
│↓ download 24.8 MiB/s (208 Mbit/s)                                                              total 84.2 GiB     │
│24.8 MiB/s                                                                                                    64M/s │
│⣀⣀⣀⣀⣀⣀⣀⣀⣀⣀⣀⣀⣀⣀⣀⣀⣀⣀⣀⣀⣀⣀⣀⣀⣀⣀⣀⣀⣀⣀⣀⣀⣀⣀⣀⣀⣀⣀⣀⣀⣀⣀⣀⣀⣀⣀⣀⣀⣀⣀⣀⣀⣀⣀⣀⣀⣀⣀⣀⣀⣀⣀⣀⣀⣀⣀⣀⣀⣀⣀⣀⣀⣀⣀⣀⣀⣀⣀⣀⣀⣀⣀⣀⣀⣀⣀⣀⣀⣀⣀⣀⣀⣀⣀⣀⣀⣀⣀⣀⣀⣀⣀⣀     0 │
│↑ upload 1.2 MiB/s (9.6 Mbit/s)                                                                  total 8.4 GiB      │
╰──────────────────────────────────────────────────────────────────────────────────────────────────────────────────────╯
╭─ proc 186/186  cpu ↓  12 run─────────────────────────────────────────────────────────────────────────────────────────╮
│ R    812 firefox            /usr/lib/firefox/firefox                                     you          1.4G ██▍  12.4 │
│ S   1021 terminal           /usr/bin/alacritty                                           you          184M █▏    5.8 │
│ S   1044 code               /usr/share/code/code --type=renderer                        you          912M █▏    4.1 │
│ S      1 systemd            /usr/lib/systemd/systemd --switched-root --system            root         14M ░     0.0 │
╰──────────────────────────────────────────────────────────────────────────────────────────────────────────────────────╯
 ↑↓ 选择  ←→ 排序  Enter 详情  f 过滤  r 反向  t 树形  k 信号  ? 帮助  q 退出
```

布局会随终端尺寸自动调整。窄终端会自动切换为紧凑视图；按 `1`–`5` 可单独查看某个面板。

## 平台支持

| 能力 | Linux | macOS | Windows |
| --- | :---: | :---: | :---: |
| CPU、每核心、频率、负载、运行时间 | 支持 | 支持 | 支持 |
| 内存与交换分区 | 支持 | 支持 | 支持 |
| 磁盘占用与 I/O | 支持 | 支持 | 支持 |
| 网络速率与累计流量 | 支持 | 支持 | 支持 |
| 进程、排序、过滤、树形 | 支持 | 支持 | 支持 |
| 进程线程数 | 支持 | 暂不提供 | 暂不提供 |
| 发送进程信号 | Unix 信号 | Unix 信号 | TerminateProcess |
| CPU 温度 | sysinfo 传感器 | sysinfo 传感器 | sysinfo 传感器* |
| 电池指示 | sysfs | 暂未实现 | `GetSystemPowerStatus` |
| NVIDIA GPU | NVML（占用/显存/温度/功耗/频率） | 暂未实现 | NVML |
| AMD / Intel GPU | sysfs（占用/显存/温度/功耗/频率） | 暂未实现 | 暂未实现 |

\* 温度取决于固件/驱动是否向系统暴露传感器；没有传感器时只隐藏温度指示，不影响其他功能。

默认使用 Unicode braille 与制表符。如果终端或字体不支持，可使用 `--tty` 切换到
纯 ASCII 边框、进度条、箭头和 `#` 图形。

## 安装

要求：Rust 1.85 或更高版本，以及支持 UTF-8 的终端。推荐 Windows Terminal、iTerm2、
WezTerm、Alacritty、Kitty、GNOME Terminal、Konsole 等。

从仓库安装：

```bash
cargo install --path .
rtop
```

或直接从 Git 安装：

```bash
cargo install --git https://github.com/summertools/rtop.git
```

本地构建：

```bash
cargo build --release
./target/release/rtop
```

发布压缩包中只有一个可执行文件：`rtop`（Windows 上为 `rtop.exe`）。发布流程会生成
`rtop-linux-x86_64.tar.gz`、`rtop-macos-aarch64.tar.gz`、
`rtop-macos-x86_64.tar.gz`、`rtop-windows-x86_64.zip`，以及用于完整性校验的
`SHA256SUMS` 文件。

## 使用

```text
rtop [OPTIONS]
```

| 选项 | 说明 |
| --- | --- |
| `-p, --preset <0-5>` | 启动时显示总览、CPU、内存、网络、进程或 GPU 视图 |
| `-u, --update <MS>` | 刷新间隔，`250`–`10000` 毫秒，默认 `1000` |
| `--theme <NAME>` | 内置主题名或用户主题文件 |
| `--graph-symbol <STYLE>` | `braille`、`block` 或 `tty` |
| `-t, --tty` | 强制 16 色和 ASCII 友好输出 |
| `--low-color` | 将颜色转换为 256 色 |
| `--no-color` | 完全禁用颜色（选中项使用反色） |
| `--no-mouse` | 不捕获鼠标 |
| `--read-only` | 禁止发送进程信号 |
| `--config <FILE>` | 使用指定配置文件 |
| `--dump` | 输出一帧纯文本后退出 |
| `--width <COLS>` | `--dump` 的宽度，默认 `120` |
| `--height <ROWS>` | `--dump` 的高度，默认 `36` |
| `--list-themes` | 列出内置和用户主题 |
| `-h, --help` / `-V, --version` | 帮助 / 版本 |

`--dump` 不需要终端，非常适合在服务器或 CI 中验证构建：

```bash
rtop --dump --width 100 --height 30 --preset 1
```

### 环境变量

| 变量 | 作用 |
| --- | --- |
| `NO_COLOR` | 禁用颜色（等同于 `--no-color`） |
| `CLICOLOR` | `CLICOLOR=0` 禁用颜色 |
| `COLORTERM` | 包含 `truecolor` 或 `24bit` 时启用 24 位色 |
| `TERM` | 包含 `256color` 时使用 256 色；交互模式下拒绝 `dumb` |
| `TERM_PROGRAM`、`WT_SESSION` | 帮助识别真彩色终端 |
| `USER`、`USERNAME` | 头部用户名的后备来源 |

## 交互按键

| 按键 | 操作 |
| --- | --- |
| `↑`、`↓`、`j` | 移动进程选择 |
| `PgUp`、`PgDn` | 翻页 |
| `Home`、`End`、`g`、`G` | 第一项 / 最后一项 |
| `Enter`、`i` | 进程详情 |
| `f` | 过滤进程（输入后 `Enter` 应用，`Esc` 取消，`Ctrl+U` 清空） |
| `←`、`→` | 上一个 / 下一个进程排序字段 |
| `c`、`m`、`p`、`n`、`u` | 按 CPU、内存、PID、名称、用户排序 |
| `r` | 反转排序 |
| `t` | 切换树形视图 |
| `e` / `w` / `Space` | 展开 / 折叠子树 |
| `k` | 发送信号（带确认对话框） |
| `0`–`5`、`Tab`、`Shift+Tab` | 切换视图 |
| `[`、`]` | 上一个 / 下一个网卡 |
| `a` | 切换自动选择网卡 |
| `+`、`-` | 加快 / 减慢刷新 |
| `s` | 暂停 / 恢复更新 |
| `F1`、`?` | 帮助 |
| `F2` | 循环切换内置主题 |
| `x` | 清除进程过滤 |
| `Esc` | 关闭弹层、清除过滤或返回总览 |
| `q`、`Ctrl+C` | 退出 |

鼠标可点击视图标签、进程行（双击打开详情）、网卡行，并可在进程列表上滚动滚轮。

## 配置文件

配置文件可选；不存在时使用默认值。位置：

| 平台 | 路径 |
| --- | --- |
| Linux | `$XDG_CONFIG_HOME/rtop/rtop.toml`，否则 `~/.config/rtop/rtop.toml` |
| macOS | `~/Library/Application Support/rtop/rtop.toml` |
| Windows | `%APPDATA%\rtop\rtop.toml` |

示例：

```toml
# 刷新间隔（毫秒，250-10000）
update_ms = 1000

# 0 总览，1 CPU，2 内存/磁盘，3 网络，4 进程，5 GPU
preset = 0

# braille、block 或 tty
graph_symbol = "braille"

# 内置主题名，或 themes 目录中的 .theme 文件
theme = "default"

truecolor = true
tty_mode = false
rounded_corners = true
theme_background = false
show_battery = true
show_gpu = true         # 检测到设备时显示 GPU 视图与摘要

proc_sort = "cpu"       # cpu、memory、pid、name、user
proc_tree = false
proc_reversed = false

net_auto = true
net_iface = ""          # net_auto = false 时优先使用的网卡
mouse = true
read_only = false
show_key_hints = true
```

非法值会被修正或忽略并给出警告；配置格式错误不会阻止程序启动。未知字段会被忽略，
以保持向前兼容。配置文件和主题文件最大读取 128 KiB。

## 主题

内置主题：`default`（Tokyo Night）、`dracula`、`gruvbox`、`nord`、`catppuccin`、
`light`、`tty`。

用户主题放在平台配置目录下的 `rtop/themes/`（例如 Linux 的
`~/.config/rtop/themes/`、macOS 的
`~/Library/Application Support/rtop/themes/`，或 Windows 的
`%APPDATA%\rtop\themes\`），语法与 btop 主题一致：

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

主题名只允许 ASCII 字母、数字、`-` 和 `_`，以防止路径穿越。未知键和非法颜色会被忽略。
可以把现有 btop 主题复制到主题目录，然后通过 `--theme mytheme` 使用。

## 安全与健壮性

`rtop` 把从其他进程读到的数据视为不可信输入：

- **终端注入防护**：进程名、命令行、用户名、挂载点、网卡名和地址在绘制前都会经过
  ANSI/控制字符/双向控制字符转义。ESC、C0/C1 控制符、双向覆盖字符和零宽方向标记
  无法改变界面布局。
- **不执行 shell**：程序不会调用 shell；信号通过操作系统 API 发送。GPU 数据来自
  有界 sysfs 读取和本地安装的 NVML 库，不会执行 `nvidia-smi`。
- **PID 复用防护**：发送信号前会重新读取进程启动时间和名称，与确认框展示的快照比对。
  Unix 上拒绝 PID 1，Windows 上拒绝低编号系统进程，并拒绝向自身发送信号。
  `--read-only` 会完全禁用信号对话框。
- **有界读取**：配置和主题文件限制为 128 KiB，主题名白名单校验；Linux sysfs 读取
  限制大小，构造路径前校验网卡名。
- **终端恢复**：原始模式、备用屏幕、鼠标捕获和 bracketed paste 会通过 RAII、panic
  hook 以及 `SIGTERM`/`SIGHUP`/`SIGQUIT` 处理器恢复。
- **最少 unsafe**：rtop 自身只有三个小型平台调用使用 `unsafe`（Unix 的 `localtime_r`，
  Windows 的 `GetLocalTime` 与 `GetSystemPowerStatus`）。GPU 数据通过安全的
  `nvml-wrapper` 动态加载驱动，其 FFI 由该库自行封装。
- **无网络访问**、无后台服务。所有数据只从操作系统读取，进程退出后即丢弃。

发送信号是明确的、需要确认的操作，并受操作系统权限限制。在共享或不可信机器上，
可以使用 `--read-only` 获得严格只读的监视器。

## 工作原理

```text
┌──────────────┐   Snapshot   ┌──────────────┐   Canvas   ┌──────────────┐
│  Collector   │ ───────────▶ │     App      │ ─────────▶ │   Renderer   │
│ (系统采样)   │   有界通道    │ 状态与输入    │   帧差分    │ (crossterm)  │
└──────────────┘              └──────────────┘            └──────────────┘
```

- 后台采集线程按配置间隔采样，并通过有界通道发送拥有所有权的 `Snapshot`。
  UI 线程不会因为 `/proc`、WMI 或 IOKit 阻塞。
- 平台数据被规范化为普通 Rust 结构体（`CpuSnapshot`、`MemorySnapshot`、
  `DiskSnapshot`、`NetworkSnapshot`、`GpuSnapshot`、`ProcessInfo`）。Linux 额外提供
  CPU 时间拆分、缓存内存、网卡速率/无线状态、GPU sysfs 数据和电池信息。
- UI 绘制到带裁切的单元格画布中。宽字符显式处理，边框和图形都做越界检查，
  并与上一帧比较，只向终端发送变化的单元格。
- 历史数据保存在 App 而不是采集器中，因此暂停更新会冻结视图，而不是丢数据。

## 开发

```bash
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo test
cargo build --release
```

CI 使用的跨平台检查：

```bash
cargo check --target aarch64-apple-darwin
cargo check --target x86_64-pc-windows-gnu
```

测试覆盖格式化与宽度处理、转义、主题解析、配置校验、图形缩放、画布宽字符修复、
进程排序/树构建/过滤、选择行为、信号安全、从 40×12 到 200×60 的所有视图、
弹层以及端到端 CLI 行为。

## 项目结构

| 路径 | 作用 |
| --- | --- |
| `src/main.rs` | 程序入口、采集线程、信号、事件循环 |
| `src/app.rs` | 应用状态、进程列表、动作与输入映射 |
| `src/cli.rs` | 命令行接口与覆盖项 |
| `src/config.rs` | 配置加载、校验、主题发现 |
| `src/format.rs` | 字节/速率/时长格式化、宽度截断、转义 |
| `src/platform/` | 跨平台快照采集 + Linux/Windows 扩展 |
| `src/theme.rs` | 颜色模型、内置主题、btop 主题解析 |
| `src/term.rs` | 终端生命周期、差分渲染、颜色量化 |
| `src/ui/` | 画布、图形、控件、面板、弹层、自适应布局 |
| `tests/cli.rs` | 端到端 CLI 测试 |
| `tests/render.rs` | 布局、溢出与注入防护冒烟测试 |

## 已知限制

- GPU 监控为只读且尽力而为：NVIDIA 需要可用的 NVML 驱动，AMD/Intel 依赖内核暴露
  DRM/hwmon 文件；暂不支持 Apple GPU、Windows 上的 AMD/Intel GPU、超频与风扇控制。
- 电池状态支持 Linux（sysfs）和 Windows（`GetSystemPowerStatus`），macOS 暂未实现。
- 便携式进程后端在 macOS 和 Windows 上不提供线程数；详情面板显示 `-`。
- 温度传感器取决于操作系统暴露的硬件信息。容器和虚拟机通常没有传感器。
- 指标是刷新间隔上的快照，不是硬实时数据流。进程数量极大时单次采样可能变慢，
  但采集运行在独立线程，不影响输入响应。
- 磁盘列表来自平台挂载的文件系统。绑定挂载和伪文件系统会按设备与容量去重/过滤，
  但特殊存储栈仍可能显示不常见的挂载名。
- 交互界面需要终端。脚本请使用 `--dump`。

## 许可证

MIT，详见 [LICENSE](LICENSE)。

设计灵感来自 [btop++](https://github.com/aristocratos/btop)；本项目是独立的 Rust 实现，
不共享其源码。
