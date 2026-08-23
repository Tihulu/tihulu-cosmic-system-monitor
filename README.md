# Tihulu COSMIC System Monitor

A small native **COSMIC panel applet** for Pop!_OS/COSMIC that shows the important hardware stats directly in the panel:

- **CPU:** utilization (%) and temperature (°C)
- **GPU:** utilization (%) and temperature (°C)
- **RAM:** used / total GiB
- **VRAM:** used / total GiB
- Refreshes every **1 second**
- NVIDIA support through `nvidia-smi`
- AMD/DRM fallback through Linux sysfs where available
- No monitoring daemon or background service

The panel text looks roughly like:

```text
CPU  18% 52°C   GPU   3% 41°C   RAM 12.7/62.6G   VRAM 2.1/15.9G
```

## One-line install

```bash
curl -fsSL https://raw.githubusercontent.com/Tihulu/tihulu-cosmic-system-monitor/main/scripts/quick-install.sh | bash
```

The installer installs the build dependencies, builds the Rust applet, runs the tests, and installs the applet under `/usr`.

After installation, open **Settings → Desktop → Panel** and add **Tihulu System Monitor**. If it does not appear immediately, restart the COSMIC session or log out and back in.

## Requirements

- Pop!_OS / COSMIC Desktop
- Rust toolchain (the quick installer installs Rust when it is missing)
- COSMIC/libcosmic build dependencies
- For NVIDIA GPU/VRAM values: a working `nvidia-smi` from the NVIDIA driver package

CPU usage and RAM are read directly from `/proc`. CPU temperature is read from Linux hwmon/thermal sensors. NVIDIA GPU usage, temperature and VRAM are queried with:

```bash
nvidia-smi --query-gpu=utilization.gpu,temperature.gpu,memory.used,memory.total --format=csv,noheader,nounits
```

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

From a clone of the repository:

```bash
sudo just uninstall
```

or:

```bash
curl -fsSL https://raw.githubusercontent.com/Tihulu/tihulu-cosmic-system-monitor/main/scripts/uninstall.sh | bash
```

## Installer options

The quick installer supports the same style of overrides as the Tihulu clipboard applet:

```bash
curl -fsSL https://raw.githubusercontent.com/Tihulu/tihulu-cosmic-system-monitor/main/scripts/quick-install.sh \
  | BRANCH=main PREFIX=/usr KEEP_BUILD_DIR=1 bash
```

`REPO_URL` can also be overridden for forks.

## License

GNU Affero General Public License v3.0 (AGPLv3). See [LICENSE](LICENSE).
