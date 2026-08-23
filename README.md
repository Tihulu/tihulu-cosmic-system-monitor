# Tihulu COSMIC System Monitor

A native **COSMIC panel applet** for Pop!_OS/COSMIC that keeps useful hardware statistics visible in the panel and opens a detailed live dashboard when clicked.

## Install / update — one line

```bash
curl -fsSL https://raw.githubusercontent.com/Tihulu/tihulu-cosmic-system-monitor/stable/scripts/quick-install.sh | bash
```

The one-line installer always installs the **CI-verified `stable` branch**. New changes are developed on `main`; only after GitHub Actions finishes successfully does CI automatically promote that exact commit to `stable`. If CI is red, the installer keeps using the previous known-good build.

After installation, open **Settings → Desktop → Panel** and add **Tihulu System Monitor**. If an older copy is already running, remove/re-add the applet or log out and back in.

## Panel

The panel can show any combination of:

- **CPU:** utilization (%) and temperature (°C)
- **GPU:** utilization (%) and temperature (°C)
- **RAM:** used / total GiB
- **Swap:** used / total GiB
- **VRAM:** used / total GiB
- **Network:** current download / upload rate

Everything refreshes every **1 second**. Swap is shown by default next to RAM.

**Left-click** opens the detailed dashboard. **Right-click** opens the panel display menu, where CPU, GPU, RAM, Swap, VRAM and Network can be enabled or disabled individually. The choices are saved in `~/.config/tihulu-cosmic-system-monitor/panel.conf`.

## Detailed dashboard

The dashboard uses tabs for **Overview, CPU, GPU, Memory and Network**, with a wider popup for readable metrics. Long sections can still scroll vertically.

- 60-sample history graphs for CPU usage and CPU temperature
- 60-sample history graphs for GPU usage and GPU temperature
- RAM, Swap and VRAM usage history
- Network download/upload history and current RX/TX rates
- CPU model, physical/logical core count, average clock, load average and uptime
- Per-logical-core CPU usage with live utilization bars
- GPU model, NVIDIA driver, power draw / power limit, core and memory clocks
- RAM, Swap and VRAM detailed usage
- Active network interfaces

The applet samples continuously, so history keeps filling even while the popup is closed.

## Data sources

No monitoring daemon is required.

- CPU usage and per-core usage: `/proc/stat`
- CPU model / frequency: `/proc/cpuinfo`
- CPU temperature: `/sys/class/hwmon` with thermal-zone fallback
- RAM / Swap: `/proc/meminfo`
- Network: `/proc/net/dev`
- NVIDIA GPU: `nvidia-smi`
- AMD/DRM fallback: Linux DRM/sysfs where available

## Requirements

- Pop!_OS / COSMIC Desktop
- Rust toolchain (the quick installer installs/updates Rust when needed)
- COSMIC/libcosmic build dependencies
- For full NVIDIA GPU/VRAM/power/clock data: a working `nvidia-smi`

## Manual build

```bash
git clone https://github.com/Tihulu/tihulu-cosmic-system-monitor.git
cd tihulu-cosmic-system-monitor
cargo test --all-targets
cargo build --release
sudo install -Dm0755 target/release/tihulu-cosmic-system-monitor /usr/bin/tihulu-cosmic-system-monitor
sudo install -Dm0644 resources/app.desktop /usr/share/applications/io.github.tihulu.SystemMonitor.desktop
sudo install -Dm0644 resources/app.metainfo.xml /usr/share/metainfo/io.github.tihulu.SystemMonitor.metainfo.xml
sudo install -Dm0644 resources/icon.svg /usr/share/icons/hicolor/scalable/apps/io.github.tihulu.SystemMonitor.svg
```

## Install a development ref

```bash
curl -fsSL https://raw.githubusercontent.com/Tihulu/tihulu-cosmic-system-monitor/stable/scripts/quick-install.sh \
  | REF=<branch-tag-or-commit> bash
```

## Uninstall

```bash
curl -fsSL https://raw.githubusercontent.com/Tihulu/tihulu-cosmic-system-monitor/stable/scripts/uninstall.sh | bash
```

## License

GNU Affero General Public License v3.0 (AGPLv3). See [LICENSE](LICENSE).
