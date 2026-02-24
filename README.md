# Themewalker

A terminal user interface for browsing and applying [SDDM](https://github.com/sddm/sddm) display manager themes, written in Rust.

Navigate the list of installed themes with your keyboard, confirm your selection, and the tool writes the change directly to your SDDM configuration — escalating to `sudo` automatically when the config file requires root access.

---

## Contents

- [Preview](#preview)
- [Requirements](#requirements)
- [Installation](#installation)
  - [Pre-built binary](#pre-built-binary)
  - [Build from source](#build-from-source)
- [Usage](#usage)
  - [Keybindings](#keybindings)
  - [How themes are applied](#how-themes-are-applied)
- [Configuration](#configuration)
  - [Theme discovery](#theme-discovery)
  - [Config file locations](#config-file-locations)
- [Contributing](#contributing)
- [License](#license)

---

## Preview

```
┌─ Themewalker Theme Changer ──────────────────────────────────────────────┐
│  Config: /etc/sddm.conf                    Current: breeze               │
└──────────────────────────────────────────────────────────────────────────┘
┌─ Installed Themes (4 found) ─────────────────────────────────────────────┐
│ >> breeze — KDE Breeze                              [active]              │
│    maya — Maya                                                            │
│    sugar-candy — A community theme for SDDM                               │
│    aerial — Aerial                                                        │
└──────────────────────────────────────────────────────────────────────────┘
┌──────────────────────────────────────────────────────────────────────────┐
│           [↑/↓ k/j] Navigate    [Enter] Select    [q / Esc] Quit        │
└──────────────────────────────────────────────────────────────────────────┘
```

Pressing `Enter` on a theme opens a confirmation popup:

```
            ┌─ Confirm ──────────────────────────────────────┐
            │                                                 │
            │  Apply theme  sugar-candy  ?                    │
            │  by Marian Metzger                              │
            │                                                 │
            │  [Enter / y]  Confirm                           │
            │  [Esc   / n]  Cancel                            │
            │                                                 │
            │  (sudo may be required to write config)         │
            └─────────────────────────────────────────────────┘
```

---

## Requirements

- **Linux** with SDDM installed
- Themes installed under `/usr/share/sddm/themes/` (each theme is a subdirectory)
- `sudo` available if your user does not own `/etc/sddm.conf` directly

To **build from source** you also need:

- Rust 1.70 or later — install via [rustup](https://rustup.rs)

---

## Installation

### Pre-built binary

Download the archive for your architecture from the [Releases](../../releases/latest) page.

| Archive | Target | Notes |
|---|---|---|
| `themewalker-<version>-x86_64-linux-gnu.tar.gz` | x86\_64 glibc | Standard 64-bit, most distros |
| `themewalker-<version>-x86_64-linux-musl.tar.gz` | x86\_64 musl | Fully static, no glibc required |
| `themewalker-<version>-arch64-linux-gnu.tar.gz` | AArch64 | 64-bit ARM (Raspberry Pi 4+, ARM servers) |

**Verify the download** using the `SHA256SUMS.txt` file attached to the same release:

```bash
sha256sum --check --ignore-missing SHA256SUMS.txt
```

**Extract and install:**

```bash
tar -xzf themewalker-<version>-x86_64-linux-musl.tar.gz
sudo install -m 755 themewalker /usr/local/bin/
```

### Build from source

```bash
git clone https://github.com/DrewBornt/themewalker.git
cd themewalker
cargo build --release
```

The compiled binary is at `target/release/themewalker`. To install it system-wide:

```bash
sudo install -m 755 target/release/themewalker /usr/local/bin/
```

Or install directly into your Cargo bin directory (no `sudo` needed):

```bash
cargo install --path .
```

---

## Usage

Run the tool from any terminal:

```bash
themewalker
```

The TUI opens in an alternate screen (your existing terminal session is preserved). Use the keyboard to navigate, select a theme, and confirm. After you confirm, the TUI exits and the theme is written to the config file in your normal terminal — you will see the `sudo` password prompt here if it is required.

Once the tool exits you will see something like:

```
Applying theme 'sugar-candy'…
Config path: /etc/sddm.conf
[sudo] password for alice:
Done.  Restart SDDM (or log out) for the change to take effect.
```

**You must restart SDDM for the change to take effect.** On most systems:

```bash
sudo systemctl restart sddm
```

Or simply log out — the new theme will be active at the next login screen.

### Keybindings

| Key | Action |
|---|---|
| `↑` / `k` | Move selection up |
| `↓` / `j` | Move selection down |
| `Enter` | Open confirmation dialog |
| `y` / `Enter` | Confirm and apply theme *(in dialog)* |
| `n` / `Esc` | Cancel dialog / quit |
| `q` / `Esc` | Quit without making changes *(in list)* |

### How themes are applied

1. The TUI exits cleanly and restores your terminal.
2. The selected theme name is written into the `[Theme]` section of your SDDM config as `Current=<theme>`.
3. If the config file is not writable by the current user, the tool automatically re-writes it via `sudo tee`, so the `sudo` password prompt appears in your normal terminal (never inside the TUI).
4. If the `[Theme]` section or `Current=` key is missing from the config, it is created. All other config values are left untouched.

---

## Configuration

### Theme discovery

Themes are read from `/usr/share/sddm/themes/`. Each subdirectory is treated as a theme. If a `metadata.desktop` file exists inside the directory, its `Description=` and `Author=` fields are shown in the UI.

Popular theme packages for common distributions:

```bash
# Arch Linux / Manjaro
sudo pacman -S sddm-sugar-candy-git        # AUR

# Fedora
sudo dnf install sddm-breeze              # usually installed with KDE

# Ubuntu / Debian
sudo apt install --no-install-recommends sddm-theme-breeze
```

You can also install themes manually by placing them in `/usr/share/sddm/themes/` (requires root).

### Config file locations

Themewalker checks the following paths (in order) to find the active theme setting:

| Path | Notes |
|---|---|
| `/etc/sddm.conf` | Legacy single-file config |
| `/etc/sddm.conf.d/*.conf` | Modern drop-in directory; files are checked alphabetically |

Changes are written back to whichever file the current theme was read from. If no config file exists yet, `/etc/sddm.conf` is created.

---

## Contributing

Bug reports and pull requests are welcome.

```bash
# Run the test suite
cargo test

# Check for warnings
cargo clippy

# Build a release binary
cargo build --release
```

The project is structured as five modules:

| File | Responsibility |
|---|---|
| `src/theme.rs` | Discover installed themes from `/usr/share/sddm/themes/` |
| `src/config.rs` | Parse and write the SDDM INI config; sudo escalation |
| `src/app.rs` | Application state, navigation, key handling |
| `src/ui.rs` | ratatui draw functions and layout |
| `src/main.rs` | Terminal setup, event loop, post-TUI apply |

**Releases** are published automatically by the GitHub Actions workflow in `.github/workflows/release.yml` when a version tag is pushed:

```bash
git tag v1.0.0
git push --tags
```

---

## License

MIT — see [LICENSE](LICENSE).
