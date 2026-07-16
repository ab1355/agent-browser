# Resource Sizing Guide

This guide provides recommended system resource requirements and optimization patterns for deploying `agent-browser` under various usage tiers, from single-agent setups to large-scale, high-concurrency multi-agent environments.

## Architecture and Resource Footprint

To accurately size your environment, it is important to understand how `agent-browser` allocates resources:

1. **The Native Daemon (Rust)**: Built on a lightweight, asynchronous Rust architecture. The daemon acts as a highly optimized, direct-to-CDP relay. It has a remarkably tiny footprint:
   - **RAM**: ~7 MB RSS (Resident Set Size), optimized for rapid startups and negligible overhead.
   - **CPU**: Near 0% utilization when idle.

2. **The Browser (Chrome / Chromium)**: Chrome is a multi-process browser that spawns separate processes for renderers, network services, storage, and utility tasks. Chrome dominates the system resource consumption:
   - **Memory**: A fresh, headless Chrome instance starts at ~150 MB of RSS. However, modern JavaScript-heavy web applications, rich dashboards, or large Single Page Applications (SPAs) will quickly scale this from 400 to 800 MB RSS per active session.
   - **CPU**: High-concurrency browser automation is CPU-intensive, especially on pages with heavy animations, streaming data, WebGL/WebGPU, or when utilizing software-rasterization fallback (SwiftShader) in headless Linux containers without a physical GPU.

## Resource Requirements by Usage Tier

The following table outlines the recommended resource allocations based on the number of concurrent agents (active browser sessions) running in your environment.

| Usage Tier | Concurrent Agents | Recommended vCPUs | Recommended RAM | Recommended Disk Space (Ephemeral) | Primary Use Case |
| :--- | :--- | :--- | :--- | :--- | :--- |
| **Minimal (Light)** | 1–2 | 2 vCPUs | 2 GB | 5 GB | Single-agent workflows, lightweight scraping, simple form-filling, or testing. |
| **Moderate (Medium)** | 3–10 | 4–8 vCPUs | 8–16 GB | 10–20 GB | Parallel testing suites, multiple simultaneous agents, monitoring dashboards. |
| **Heavy (Scale)** | 11–50+ | 16–64+ vCPUs | 32–128+ GB | 50+ GB | Enterprise-grade multi-agent frameworks, dense scraping pipelines, high-frequency tasks. |

### CPU Allocation Guidelines
- **Minimum**: Allocate at least **0.5 to 1 vCPU per active agent**.
- **Recommended**: For heavy workloads (complex Single Page Applications, multi-tab automation, or visual processing), allocate **1.5 to 2 vCPUs per active agent** to prevent pages from timing out during CPU-bound tasks like accessibility tree (AXTree) snapshotting.

### Memory Allocation Guidelines
- **Minimum**: Allocate **1 GB of RAM per concurrent agent** plus 1 GB of system headroom.
- **Recommended**: Under heavy usage, allocate **2 GB of RAM per concurrent agent** to provide adequate headroom for memory leaks, extensive DOM trees, or media-heavy sites.

---

## High-Efficiency Optimization Patterns

To scale `agent-browser` efficiently across many agents, incorporate the following optimization patterns:

### 1. Auto-Optimized Browser Launch Arguments
`agent-browser` comes pre-configured with memory and CPU-saving flags that match the high-efficiency requirements of headless servers and containers. When launched, the browser disables unnecessary processes automatically.

*Note: Memory savings are typical reference measurements (e.g., under headless Linux) and may vary depending on the operating system, driver configuration, and Chrome version.*

- **Audio Process Muting** (`--mute-audio`): Prevents Chrome from initializing audio pipelines, saving approximately 20 MB of RAM and avoiding unnecessary utility processes.
- **GPU Suppression** (`--disable-gpu`): When WebGPU is not requested, the GPU process is suppressed to save up to 100 MB of RAM and eliminate driver initialization overhead on headless servers.
- **Shared Memory Cache** (`--disable-dev-shm-usage`): Automatically used in CI, Docker, and Podman containers to write shared memory to disk instead of `/dev/shm`, preventing random tab crashes due to small shm limits.
- **Sandbox Controls** (`--no-sandbox`): Automatically active in containerized or root environments where namespaces are restricted, reducing overhead.

### 2. JavaScript Engine (V8) Memory Limits
For extremely resource-constrained servers or dense agent environments, you can cap the maximum heap memory allocated to each Chrome renderer tab. This prevents runaway memory leaks in long-running agent loops:
- Pass V8 heap limiting options via `--args`:
  ```bash
  agent-browser --args "--js-flags=\"--max-old-space-size=512\""
  ```
  This limits the JavaScript memory space of each tab to 512 MB, allowing you to pack more agents onto a single virtual machine.

### 3. Ephemeral State & Disk I/O Management
Every time an agent starts a session without specifying a profile, a new user data directory is created in `/tmp`. This writes tens of megabytes of configuration and caches. To optimize startup latency and avoid excessive disk I/O:
- **Use tmpfs**: Mount `/tmp` as an in-memory file system (tmpfs). This speeds up browser cold starts dramatically.
- **Profile Reuse**: Use `--profile` with a persistent or shared profile directory when agents are repeatedly visiting the same platforms (see the `--profile` option in `agent-browser --help` or `README.md` for details). This caches assets, cookies, and authentication state, reducing the network and CPU load of repeated authentication flows.

### 4. Direct Visual and Accessibility-First Interaction
Minimize the extraction of heavy raw HTML or visual screenshots. Instead, leverage `agent-browser`’s optimized accessibility tree (AXTree) snapshots.
- Use **compact snapshots** (`snapshot --compact` or `snapshot -c`) to reduce the serialized node size, which minimizes daemon memory footprint and reduces token overhead in LLM context windows.
