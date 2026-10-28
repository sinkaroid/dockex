use crate::compose;
use crate::config;
use crate::prompt;
use std::path::Path;
use std::process::{Command, Stdio, exit};

fn is_container_environment() -> bool {
    Path::new("/.dockerenv").exists() || Path::new("/run/.containerenv").exists()
}

fn run_sync(command: &str) -> Result<(), (i32, String)> {
    #[cfg(target_os = "windows")]
    let mut cmd = {
        let mut c = Command::new("cmd");
        c.args(["/C", command]);
        c
    };

    #[cfg(not(target_os = "windows"))]
    let mut cmd = {
        let shell = if Path::new("/bin/bash").exists() {
            "/bin/bash"
        } else {
            "/bin/sh"
        };
        let mut c = Command::new(shell);
        c.args(["-c", command]);
        c
    };

    cmd.stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit());

    match cmd.status() {
        Ok(status) => {
            if status.success() {
                Ok(())
            } else {
                let code = status.code().unwrap_or(1);
                Err((code, format!("Command failed: {command}")))
            }
        }
        Err(err) => Err((1, format!("{err}"))),
    }
}

fn flush_memory(step_num: u8, desc: &str, skip_flush: bool) {
    if skip_flush {
        println!("\n⏩ [{step_num}/7] Skipping memory cache flush (--skip-flush active)");
        return;
    }

    if !cfg!(target_os = "linux") {
        println!(
            "\n⏩ [{step_num}/7] Skipping memory cache flush (non-Linux platform: {})",
            std::env::consts::OS
        );
        return;
    }

    let drop_caches_path = Path::new("/proc/sys/vm/drop_caches");
    if is_container_environment() || !drop_caches_path.exists() {
        println!(
            "\n🧠 [{step_num}/7] Kernel cache flush unavailable in current environment (read-only /proc or container). Proceeding..."
        );
        return;
    }

    println!("\n🧠 [{step_num}/7] {desc}...");

    // PRD Goal: Zero fatal failures during memory flush optimization
    let _ = Command::new("sync").status();

    let status = Command::new("sh")
        .arg("-c")
        .arg("echo 3 | sudo tee /proc/sys/vm/drop_caches && free -h")
        .status();

    if let Ok(s) = status {
        if !s.success() {
            println!("⚠️  [{step_num}/7] Cache flush was not permitted. Continuing deployment safely...");
        }
    }
}

pub fn execute_redeploy(explicit_engine: Option<&str>, skip_flush: bool) {
    let cwd = std::env::current_dir().unwrap_or_else(|_| Path::new(".").to_path_buf());

    // ==========================================
    // PRE-FLIGHT CHECK: Locate and inspect Compose file
    // ==========================================
    let meta = match compose::inspect_compose(&cwd) {
        Some(m) => m,
        None => {
            eprintln!(
                "\n❌ Error: No Compose file ('compose.yaml', 'compose.yml', 'docker-compose.yaml', or 'docker-compose.yml') found in:"
            );
            eprintln!("   {}\n", cwd.display());
            eprintln!(
                "Aborting redeploy workflow. Please navigate to a project containing a Compose file.\n"
            );
            exit(1);
        }
    };

    // ==========================================
    // STACK OVERVIEW: Display detected services
    // ==========================================
    if !meta.services.is_empty() {
        let count = meta.services.len();
        println!(
            "📦 Stack Overview ({count} service{} detected):",
            if count == 1 { "" } else { "s" }
        );
        let max_len = meta.services.iter().map(|s| s.name.len()).max().unwrap_or(0);
        for service in &meta.services {
            println!("  • {:<width$} → {}", service.name, service.image, width = max_len);
        }
        println!();
    }

    // ==========================================
    // STACK SELECTION: Flag > Compose x-engine > Prompt
    // ==========================================
    let loaded_config = config::load_config();
    let configured_engine = loaded_config
        .get("engine")
        .and_then(|v| v.as_str())
        .filter(|e| *e == "podman" || *e == "docker")
        .unwrap_or("docker");

    let selected_engine_buf: String;
    let engine = if let Some(eng) = explicit_engine {
        eng
    } else if let Some(ref auto_eng) = meta.engine {
        println!("📌 Engine auto-detected via 'x-engine: {auto_eng}' in compose file.");
        auto_eng.as_str()
    } else {
        selected_engine_buf = prompt::select_runtime_stack(configured_engine);
        &selected_engine_buf
    };

    println!(
        "\n🚀 Starting {} Redeployment in: {}\n",
        if engine == "podman" { "Podman" } else { "Docker" },
        cwd.display()
    );

    let handle_error = |err_code: i32, err_msg: &str| {
        if err_code == 130 || err_code == -1073741510 || err_code as u32 == 0xC000013A {
            println!("\n⚠️  Redeployment aborted by user.");
            exit(130);
        }
        eprintln!("\n❌ Workflow failed during execution: {err_msg}");
        exit(1);
    };

    // Step 1: Compose down
    println!("📦 [1/7] Stopping running services ({engine} compose down)...");
    if let Err((code, msg)) = run_sync(&format!("{engine} compose down")) {
        handle_error(code, &msg);
    }

    // Step 2: System prune with auto-confirm
    println!(
        "\n🧹 [2/7] Cleaning unused containers, images & volumes ({engine} system prune -a --volumes -f)..."
    );
    if let Err((code, msg)) = run_sync(&format!("{engine} system prune -a --volumes -f")) {
        handle_error(code, &msg);
    }

    // Step 3: Flush cache 1
    flush_memory(3, "Flushing memory cache (sync + drop_caches)", skip_flush);

    // Step 4: Pull latest images
    println!("\n📥 [4/7] Pulling latest images ({engine} compose pull)...");
    if let Err((code, msg)) = run_sync(&format!("{engine} compose pull")) {
        handle_error(code, &msg);
    }

    // Step 5: Flush cache 2
    flush_memory(5, "Flushing memory post-pull (sync + drop_caches)", skip_flush);

    // Step 6: Compose up -d
    println!("\n✨ [6/7] Starting containers in detached mode ({engine} compose up -d)...");
    if let Err((code, msg)) = run_sync(&format!("{engine} compose up -d")) {
        handle_error(code, &msg);
    }

    // Step 7: Stream logs
    println!(
        "\n📜 [7/7] Streaming container logs ({engine} compose logs -f, Press Ctrl+C to detach without stopping)...\n"
    );

    // Prevent parent process from being killed by SIGINT when user hits Ctrl+C
    #[cfg(unix)]
    unsafe {
        unsafe extern "C" {
            fn signal(sig: std::os::raw::c_int, handler: usize) -> usize;
        }
        signal(2, 1); // SIGINT, SIG_IGN
    }

    #[cfg(windows)]
    unsafe {
        unsafe extern "system" {
            fn SetConsoleCtrlHandler(
                handler: Option<unsafe extern "system" fn(u32) -> i32>,
                add: i32,
            ) -> i32;
        }
        unsafe extern "system" fn ctrl_handler(_ctrl_type: u32) -> i32 {
            1 // Handled: don't terminate parent
        }
        SetConsoleCtrlHandler(Some(ctrl_handler), 1);
    }

    let mut logs_cmd = Command::new(engine);
    logs_cmd
        .args(["compose", "logs", "-f"])
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit());

    #[cfg(unix)]
    unsafe {
        use std::os::unix::process::CommandExt;
        logs_cmd.pre_exec(|| {
            unsafe extern "C" {
                fn signal(sig: std::os::raw::c_int, handler: usize) -> usize;
            }
            signal(2, 0); // SIGINT, SIG_DFL: reset to default in child process
            Ok(())
        });
    }

    let _ = logs_cmd.status();

    println!("\n🔌 Detached from logs. Stack is running in detached mode.");
    println!("✅ Deployment workflow completed successfully.");
}
