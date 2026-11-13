# P1 COSMIC Panel memory-growth acceptance — runtime QA HOLD

## Reproduction and hypothesis

Original baseline source: `main`/`stable` commit `c5ba7f8baa888bfdf298912ad4b8389dacdd8690`.
Owner-reported Pop!_OS 24.04 COSMIC/Wayland NVIDIA RTX 5080: with three applets, `cosmic-panel` RSS exceeded 4,018,440 KiB and grew about 8–18 MiB/min with approximately 810 FDs. After all three applets were removed, the same panel PID settled near 672,080 KiB. The user's data strongly link the problem to applet activity, but do not isolate a root cause.

In the baseline `SystemMonitor::view()`, `cosmic::widget::Id::unique()` is called for the autosize widget on *every redraw*. `libcosmic`'s autosize widget exposes that ID through `Widget::id()` and passes it into `Operation::container`. The changed identity can invalidate state continuity and may trigger layout/IPC churn. The patch creates a unique ID once per applet instance and clones it on redraw. Popup `window::Id::unique()` is deliberately unchanged because each *new popup window* needs a new window identity.

For the candidate, `Cargo.toml` pins `libcosmic` to `ecc5fe4164e1c9024b932382d24ea41f24147e75`, the revision seen in the successful source-check/test/release-build CI run. The original baseline did **not** pin `libcosmic`; capture its actual resolved revision before comparing, and use the same upstream revision for controlled A/B testing where possible. Cargo registry/transitive dependencies still require recording resolved versions and build logs.\n\nOther candidates remain open: accelerated SVG icon handles, libcosmic/COSMIC Panel internals, multiple applet sampling, NVIDIA polling via `nvidia-smi`, Wayland shared memory and memfd retention. The 1-s tick remains to preserve live metrics. History queues are limited to 60 entries; graphics are constructed only when a popup view is requested. This patch is a targeted hypothesis fix, not a runtime-proven solution.

## Read-only measurements

Do **not** run this on an owner's system without separate approval. Use an authorized disposable COSMIC/Wayland runtime or a purpose-built QA workstation with matched graphics driver and libcosmic versions.

For every A/B comparison record:
- baseline and candidate exact applet commit SHAs, libcosmic resolved commit (`Cargo.lock` if present), COSMIC panel binary version/build, kernel version, NVIDIA driver version, GPU model and display scaling;
- same panel geometry, chosen metrics, refresh interval, other running applets and NVIDIA workload;
- per-PID `/proc/PID/smaps_rollup` RSS, PSS, Private_Dirty and Swap; `/proc/PID/fd` count and memfd count;
- panel PID/start time, all applet PIDs, elapsed time, popup state, settings and resize events, crash/restart logs;
- at least 600 seconds of steady state per scenario, with first/last values and ordinary-least-squares MiB/min slopes (not just endpoint differences).

`scripts/sample-cosmic-memory.sh` is a **read-only process sampler** (other than writing a new CSV) and refuses to overwrite a capture. Run it separately for the panel and each applet (example placeholders, not an authorization to install or launch anything):

```bash
bash scripts/sample-cosmic-memory.sh <PANEL_PID> 600 10 panel.csv
bash scripts/sample-cosmic-memory.sh <APPLET_PID> 600 10 applet.csv
```

Samples should be collected concurrently for multi-process tests, each under a different CSV filename. A PID exit/restart terminates that sampler with a nonzero status; document the restart separately rather than silently treating it as the same process. FDs and memfd targets are best-effort snapshots; permissions may prevent collecting them.

## Scenario matrix

1. Control: panel with no TIHULU applets, 10 minutes; then remove all test applets and observe a second 10-minute recovery window (same PID if possible).
2. One System Monitor applet, popup closed, NVIDIA enabled, 10 minutes.
3. Three independent applets, all popups closed, 10 minutes; verify independent identities and no cross-instance UI regression.
4. One applet popup open: exercise Overview, CPU, GPU, Memory, Network and PSU tabs and live 60-s graphs for 10 minutes.
5. Repeated popup open/close, including right-click settings and persisted toggles, followed by 10-minute steady state.
6. Resize the panel/dock, change edge/orientation/scale when available; record geometry changes and 10-minute post-resize steady state.
7. NVIDIA enabled versus unavailable (`nvidia-smi` fallback) in equivalent conditions, without changing the user's actual driver.
8. Remove one and then all three applets; record panel RSS/PSS stabilization and process exit.
9. On an **isolated throwaway session only**, test panel crash/restart and applet reconnection, recovery and stable saved configuration.

All scenarios must be repeated with **both** baseline and candidate in the **same** controlled environment. Keep detailed raw CSVs and an acceptance table.

## Verdict gate

- Unit tests, formatting, build and Clippy can establish compilation and identity stability; they **cannot prove** that COSMIC Panel RSS/PSS stopped growing.
- A runtime PASS requires a reproducible baseline leak and no persistent unbounded candidate RSS/PSS/Private_Dirty/Swap growth attributable to the applet over matched 10-minute windows. Analyze noisy transients and shared memory separately; FD stability alone is insufficient.
- If source-only CI is green but native Wayland runtime or comparative traces are unavailable: **RUNTIME QA HOLD**. Do not merge, promote stable, release or claim the memory leak solved.

## Installation and rollback controls

The repo's existing `scripts/quick-install.sh` accepts `REF` and downloads `https://github.com/Tihulu/tihulu-cosmic-system-monitor/archive/${REF}.tar.gz`. Its source, test, build and copy steps overwrite the executable under `PREFIX`, while preserving the separate panel configuration file. It does **not** provide an automatic binary rollback. Before any authorized test installation, record a known-good commit and back up the installed binary and corresponding desktop resources; the user must separately authorize any installation.

After owner-authorized test installation, verify the installed executable path and `pgrep -af '^tihulu-cosmic-system-monitor'`, then compare panel/process 10-minute traces. To roll back, rerun the same quick installer pinned to the recorded known-good commit (or restore the backed-up binary), and remove/re-add the applet or restart the user session if COSMIC retains the old process. Never assume an `app --version` flag exists.
