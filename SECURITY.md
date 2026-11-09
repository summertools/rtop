# Security policy

## Supported versions

| Version | Supported |
| --- | --- |
| 0.1.x | Yes |
| older | No |

## Reporting a vulnerability

Please report security issues privately through GitHub's **Security advisories**
for the repository (`Security` → `Advisories` → `Report a vulnerability`) rather
than opening a public issue. Include the affected version, platform, terminal,
and a minimal reproduction if possible.

You can expect an acknowledgement within a few days. Please allow time for a
fix and a release before public disclosure.

## Scope

`rtop` reads operating-system data and writes it to a terminal. Areas of
particular interest:

- terminal escape or bidirectional-text injection through process names,
  command lines, mount points, interface names, hostnames, or user names;
- path traversal or oversized-file issues in configuration and theme loading;
- process signal safety: PID reuse, permission bypass, or signalling the wrong
  process;
- terminal state left behind after panics or signals;
- unsafe code. rtop itself uses `unsafe` only for three small platform calls
  (`localtime_r` on Unix, `GetLocalTime` and `GetSystemPowerStatus` on Windows)
  plus the `signal-hook` dependency. GPU vendor data is read through the safe
  `nvml-wrapper` crate, whose dynamic-library FFI is encapsulated by that crate.
- GPU probing: Linux sysfs reads are bounded and restricted to fixed
  `/sys/class/drm` and `/sys/class/hwmon` paths; NVML is loaded dynamically and
  never through a shell command. On Windows it is loaded from the protected
  `System32` directory rather than the current working directory. GPU
  monitoring is read-only and offers no fan or clock control.

Out of scope: the accuracy of data reported by the operating system, denial of
service caused by a user choosing an extremely small refresh interval, and
process names or command lines that are merely surprising but correctly
rendered with sanitisation.

## Operational advice

- Run with `--read-only` on shared or untrusted machines to disable process
  signalling entirely.
- The program never needs root privileges for normal monitoring. Running it as
  root expands what it can read and signal, so prefer a normal user account.
- Use `--no-mouse` if terminal text selection is more important than mouse
  interaction.
