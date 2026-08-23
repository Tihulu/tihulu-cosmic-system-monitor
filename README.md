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

COSMIC currently constrains the practical applet popup width, so the dashboard is designed for the actual narrow popup instead of requesting a wider window. Long values are stacked below their labels, tabs are split across two rows, and long sections scroll vertically.

Tabs: **Overview, CPU, GPU, Memory, Network, PSU**.

- 60-sample history graphs for CPU usage and CPU temperature
- 60-sample history graphs for GPU usage and GPU temperature
- RAM, Swap and VRAM usage history
- Network download/upload history and current RX/TX rates
- CPU model, physical/logical core count, average clock, load average and uptime
- Per-logical-core CPU usage
- GPU model, driver, board power, clocks and VRAM
- NVIDIA GPU topology where a verified model specification is available: SMs, CUDA cores, Tensor cores, RT cores, GPC/TPC partitions, texture units, ROPs, media engines, compute capability and memory bus
- PSU / power-supply information exposed through Linux power-supply and hwmon interfaces

For the desktop **GeForce RTX 5080**, the GPU tab includes the verified GB203/Blackwell topology: 84 SMs, 10,752 CUDA cores, 336 fifth-generation Tensor cores, 84 fourth-generation RT cores, 7 GPCs, 42 TPCs, 336 texture units and 112 ROPs.

Desktop ATX PSUs normally do **not** expose their model, rated wattage, efficiency, rail telemetry or total wall draw to Linux. The PSU tab therefore shows only values the hardware/driver actually exposes and clearly marks unavailable PSU telemetry instead of guessing.

The applet samples continuously, so history keeps filling even while the popup is closed.

## Data sources

No monitoring daemon is required.

- CPU usage and per-core usage: `/proc/stat`
- CPU model / frequency: `/proc/cpuinfo`
- CPU temperature and power sensors: `/sys/class/hwmon`
- RAM / Swap: `/proc/meminfo`
- Network: `/proc/net/dev`
- PSU / AC / battery information: `/sys/class/power_supply`
- NVIDIA runtime GPU data: `nvidia-smi`
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
