# AGENTS.md

> Project operational context, conventions, and status guide for AI agents working on **dockex**.

---

## 1. Project Overview & Context

- **Project Name**: `dockex` (distributed on NPM as `@sinkaroid/dockex`)
- **Registry Target**: NPM (`https://www.npmjs.com/package/@sinkaroid/dockex`) & Crates.io (`dockex`)
- **Core Functionality**: High-performance, resilient Docker and Podman Compose redeployment lifecycle manager with automated container pruning and kernel page-cache flushing (`drop_caches`).
- **Current Status**:
  - Successfully rewritten from TypeScript to **pure Rust (2024 Edition)**.
  - Legacy TypeScript files have been removed after passing a comprehensive **1:1 behavioral parity** test suite.
  - The release binary compiles in ~2.5s, is **~278 KB** in size, and starts in <3ms.

---

## 2. Architecture & Design Principles

### A. Minimal Dependency Philosophy

Do **not** introduce heavy asynchronous runtimes (e.g., Tokio), massive CLI frameworks (e.g., Clap derive macros), or heavy terminal UI libraries.

- **CLI Parsing**: Standard library vector iteration (`std::env::args`).
- **Process & Streaming**: Native OS process spawning via `std::process::Command` with `.stdin(Stdio::inherit())`, `.stdout(Stdio::inherit())`, `.stderr(Stdio::inherit())`.
- **Interactive Prompts**: Standard IO (`std::io::stdin`, `std::io::stdout`, `io::Write::flush`).
- **Configuration**: Managed via `serde` and `serde_json` in `~/.dockex.json`, with transparent fallback reading from legacy `~/.dockex.json`, `~/.shipx.json`, and `~/.dockerized.json`.

### B. NPM Bridge Architecture

The NPM package distributes the native binary using a lightweight launcher pattern:

- **`bin/dockex.js`**: Pure vanilla Node.js (~35 lines, zero dependencies).
- When a user executes `npx @sinkaroid/dockex` or `dockex` via global npm install, `bin/dockex.js` detects `process.platform` and `process.arch`, resolves the binary path, and delegates execution using `child_process.spawnSync`.

---

## 3. Directory Layout

```
e:\Ferris\dockerized\
├── src/
│   ├── main.rs         # CLI entrypoint, argument parsing, config subcommands, and dispatch
│   ├── redeploy.rs     # 7-step deployment lifecycle, stack overview, & resilient memory flushing
│   ├── compose.rs      # Zero-dependency Compose inspector (x-engine & service/image parser)
│   ├── config.rs       # ~/.dockex.json key-value store (with fallback support)
│   ├── prompt.rs       # Interactive container engine selection (Docker vs Podman)
│   └── bin/
│       └── xtask.rs    # Pure Rust automation tool (platform packaging, update-deps, publish)
├── bin/
│   └── dockex.js       # Node.js binary launcher for NPM distribution
├── Cargo.toml          # Rust manifest (Edition 2024, size-optimized release profile)
├── package.json        # NPM package configuration (@sinkaroid/dockex launcher bin)
├── PRD.md              # Requirements document (Memory resilience & compose specifications)
├── README.md           # User documentation and usage guide
├── .gitignore          # Ignores target/, node_modules/, etc.
└── AGENTS.md           # This file
```

---

## 4. The 7-Step Lifecycle & PRD Resilience Rules

Every redeployment follows this precise sequence:

1. **Pre-flight Check & Stack Overview**:
   - Validates the presence of `compose.yaml`, `compose.yml`, `docker-compose.yaml`, or `docker-compose.yml` in `cwd`. Aborts with exit code 1 if missing.
   - Extracts service names and images/builds via zero-dependency scanner (`src/compose.rs`) and prints a formatted **Stack Overview**.
   - Resolves runtime engine: `CLI Flag > Compose x-engine > Interactive Prompt > default ('docker')`.
2. **Step 1**: `<engine> compose down` — Stop services.
3. **Step 2**: `<engine> system prune -a --volumes -f` — Purge dangling resources.
4. **Step 3**: Kernel memory cache flush (`sync` + `/proc/sys/vm/drop_caches` + `free -h`).
5. **Step 4**: `<engine> compose pull` — Fetch latest remote images.
6. **Step 5**: Secondary post-pull memory cache flush.
7. **Step 6**: `<engine> compose up -d` — Start containers detached.
8. **Step 7**: `<engine> compose logs -f` — Stream logs. Cleanly exits when user presses `Ctrl+C` (SIGINT / exit code 130) without stopping background containers.

### Memory Resilience (PRD Compliance)

- Memory flushing is an **optimization** and must **never** fail the deployment.
- On non-Linux platforms (Windows NT, macOS Darwin) or when `--skip-flush` / `--no-flush` / `-s` is passed, the flush step is skipped gracefully with an informative message.
- On Linux, if container environments (`/.dockerenv`, `/run/.containerenv`) or unprivileged/read-only `/proc/sys/vm/drop_caches` are detected, the CLI logs a non-fatal notice and continues deployment uninterrupted.

---

## 5. Development & Testing Commands

- **Compile Debug**:
  ```bash
  cargo check
  cargo build
  ```
- **Compile Release**:
  ```bash
  cargo build --release
  ```
- **Run Native Binary Directly**:
  ```powershell
  .\target\release\dockex.exe --help
  .\target\release\dockex.exe config get engine
  ```
- **Test NPM Launcher**:
  ```bash
  node bin/dockex.js --help
  ```
- **Test NPM Packaging**:
  ```bash
  npm pack --dry-run
  ```

---

## 6. Multi-Platform NPM Architecture (Pilihan 1 Implemented)

- **Platform Binary Packages**: Each target OS/arch is built by GitHub Actions and published as an independent, lightweight npm package:
  - `@sinkaroid/dockex-linux-x64`
  - `@sinkaroid/dockex-linux-arm64`
  - `@sinkaroid/dockex-darwin-x64`
  - `@sinkaroid/dockex-darwin-arm64`
  - `@sinkaroid/dockex-win32-x64`
- **Root NPM Package (`@sinkaroid/dockex`)**:
  - Contains `bin/dockex.js` launcher.
  - Declares all platform packages as `optionalDependencies` with matching versions.
  - When installed on Linux via `npm i -g @sinkaroid/dockex`, NPM downloads only `@sinkaroid/dockex-linux-x64` and resolves the binary path seamlessly.
- **CI/CD Workflow (`.github/workflows/release.yml`)**:
  - `publish-platforms` job matrix cross-compiles all target binaries and publishes each `@sinkaroid/dockex-*` platform package.
  - `publish-main-and-release` job updates root `optionalDependencies`, publishes `@sinkaroid/dockex` to NPM, creates git tag, and publishes GitHub Release with attached standalone archives.
