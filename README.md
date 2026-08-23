# Tihulu COSMIC System Monitor

A native **COSMIC panel applet** for Pop!_OS/COSMIC that keeps the most useful hardware statistics visible in the panel and opens a detailed live dashboard when clicked.

## Panel

The compact panel view shows:

- **CPU:** utilization (%) and temperature (°C)
- **GPU:** utilization (%) and temperature (°C)
- **RAM:** used / total GiB
- **VRAM:** used / total GiB
- Refreshes every **1 second**

Example:

```text
CPU 18% 52°C   GPU 3% 41°C   RAM 12.7/62.6G   VRAM 2.1/15.9G
```

## Detailed dashboard

Click the applet to open a scrollable live dashboard with:

- 60-sample history graphs for CPU usage and CPU temperature
- 60-sample history graphs for GPU usage and GPU temperature
- RAM and VRAM usage history
- Network download/upload history and current RX/TX rates
- CPU model, physical/logical core count, average clock, load average, uptime
- Per-logical-core CPU usage with live utilization bars
- GPU model, NVIDIA driver, power draw / power limit, core and memory clocks
- RAM, swap and VRAM detailed usage
- Active network interfaces

The applet samples continuously, so history keeps filling even while the popup is closed.

## One-line install / update

```bash
curl -fsSL https://raw.githubusercontent.com/Tihulu/tihulu-cosmic-system-monitor/main/scripts/quick-install.sh | bash
```

Running the same command again rebuilds and installs the newest `main` version.

After installation, open **Settings → Desktop → Panel** and add **Tihulu System Monitor**. If an older copy is already running, remove/re-add the applet or restart the COSMIC session after updating.

## Data sources

No monitoring daemon is required.

- CPU usage and per-core usage: `/proc/stat`
- CPU model / frequency: `/proc/cpuinfo`
- CPU temperature: `/sys/class/hwmon` with thermal-zone fallback
- RAM / swap: `/proc/meminfo`
- Network: `/proc/net/dev`
- NVIDIA GPU: `nvidia-smi`
- AMD/DRM fallback: Linux DRM/sysfs where available

NVIDIA details are queried with `nvidia-smi`; if it is unavailable, the applet still keeps CPU, RAM and network monitoring working and tries the DRM/sysfs GPU fallback.

## Requirements

- Pop!_OS / COSMIC Desktop
- Rust toolchain (the quick installer installs/updates Rust when needed)
- COSMIC/libcosmic build dependencies
- For full NVIDIA GPU/VRAM/power/clock data: a working `nvidia-smi`

## Manual build

```bash
git clone https://github.com/Tihulu/tihulu-cosmic-system-monitor.git
cd tihulu-cosmic-system-monitor
cargo test
just build-release
sudo just install
```

## Run without installing

```bash
just run
```

## Uninstall

```bash
curl -fsSL https://raw.githubusercontent.com/Tihulu/tihulu-cosmic-system-monitor/main/scripts/uninstall.sh | bash
```

or from a clone:

```bash
sudo just uninstall
```

## Installer options

```bash
curl -fsSL https://raw.githubusercontent.com/Tihulu/tihulu-cosmic-system-monitor/main/scripts/quick-install.sh \
  | BRANCH=main PREFIX=/usr KEEP_BUILD_DIR=1 bash
```

`REPO_URL` can also be overridden for forks.

## License

GNU Affero General Public License v3.0 (AGPLv3). See [LICENSE](LICENSE).
