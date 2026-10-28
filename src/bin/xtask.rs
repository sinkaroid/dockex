use std::env;
use std::fs;
use std::path::Path;
use std::process::{Command, Stdio, exit};

fn prepare_platform(
    platform: &str,
    arch: &str,
    version: &str,
    binary_path: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let is_windows = platform == "win32";
    let binary_name = if is_windows { "dockex.exe" } else { "dockex" };
    let package_name = format!("@sinkaroid/dockex-{platform}-{arch}");
    let target_dir = Path::new("npm").join(format!("{platform}-{arch}"));
    let bin_dir = target_dir.join("bin");

    fs::create_dir_all(&bin_dir)?;
    let dest_bin = bin_dir.join(binary_name);
    fs::copy(binary_path, &dest_bin)?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(&dest_bin)?.permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&dest_bin, perms)?;
    }

    let pkg_json = serde_json::json!({
        "name": package_name,
        "version": version,
        "description": format!("Native dockex binary for {platform}-{arch}"),
        "main": format!("bin/{binary_name}"),
        "bin": {
            "dockex": format!("bin/{binary_name}")
        },
        "files": ["bin"],
        "os": [platform],
        "cpu": [arch],
        "repository": {
            "type": "git",
            "url": "git+https://github.com/sinkaroid/dockex.git"
        },
        "license": "MIT"
    });

    fs::write(
        target_dir.join("package.json"),
        serde_json::to_string_pretty(&pkg_json)? + "\n",
    )?;

    fs::write(
        target_dir.join("README.md"),
        format!("# {package_name}\n\nPrebuilt binary of dockex for {platform}-{arch}.\n"),
    )?;

    println!("✅ Prepared platform package in {}", target_dir.display());
    Ok(())
}

fn update_deps(version: &str) -> Result<(), Box<dyn std::error::Error>> {
    let pkg_path = Path::new("package.json");
    let content = fs::read_to_string(pkg_path)?;
    let mut pkg: serde_json::Value = serde_json::from_str(&content)?;

    pkg["version"] = serde_json::Value::String(version.to_string());
    pkg["optionalDependencies"] = serde_json::json!({
        "@sinkaroid/dockex-linux-x64": version,
        "@sinkaroid/dockex-linux-arm64": version,
        "@sinkaroid/dockex-darwin-x64": version,
        "@sinkaroid/dockex-darwin-arm64": version,
        "@sinkaroid/dockex-win32-x64": version,
    });

    fs::write(pkg_path, serde_json::to_string_pretty(&pkg)? + "\n")?;
    println!("✅ Updated package.json version to {version} with optionalDependencies.");
    Ok(())
}

fn publish(pkg_dir: &str, registry_url: Option<&str>) -> Result<(), Box<dyn std::error::Error>> {
    let target_path = Path::new(pkg_dir);
    let pkg_content = fs::read_to_string(target_path.join("package.json"))?;
    let pkg: serde_json::Value = serde_json::from_str(&pkg_content)?;

    let name = pkg["name"].as_str().ok_or("Missing package name")?;
    let version = pkg["version"].as_str().ok_or("Missing package version")?;
    let target_pkg = format!("{name}@{version}");
    let registry_label = registry_url.unwrap_or("npm");

    println!("Checking if {target_pkg} is already published on {registry_label}...");

    let npm_cmd_name = if cfg!(windows) { "npm.cmd" } else { "npm" };

    let mut check_cmd = Command::new(npm_cmd_name);
    check_cmd.args(["view", &target_pkg, "version", "--json"]);
    if let Some(reg) = registry_url {
        check_cmd.args(["--registry", reg]);
    }

    if let Ok(output) = check_cmd.output() {
        if output.status.success() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            if stdout.contains(version) {
                println!("⏩ {target_pkg} is already published on {registry_label}. Skipping.");
                return Ok(());
            }
        }
    }

    println!("🚀 Publishing {target_pkg} to {registry_label}...");
    let mut pub_cmd = Command::new(npm_cmd_name);
    pub_cmd.args(["publish", "--access", "public"]);
    if let Some(reg) = registry_url {
        pub_cmd.args(["--registry", reg]);
    }
    pub_cmd.current_dir(target_path);
    pub_cmd.stdin(Stdio::inherit());
    pub_cmd.stdout(Stdio::inherit());
    pub_cmd.stderr(Stdio::inherit());

    let status = pub_cmd.status()?;
    if !status.success() {
        eprintln!("❌ Failed to publish {target_pkg} to {registry_label}");
        exit(1);
    }

    println!("✅ Successfully published {target_pkg} to {registry_label}");
    Ok(())
}

fn main() {
    let args: Vec<String> = env::args().collect();
    let command = args.get(1).map(|s| s.as_str()).unwrap_or("");

    let result = match command {
        "prepare-platform" => {
            if args.len() < 6 {
                eprintln!(
                    "Usage: cargo xtask prepare-platform <platform> <arch> <version> <binary_path>"
                );
                exit(1);
            }
            prepare_platform(&args[2], &args[3], &args[4], &args[5])
        }
        "update-deps" => {
            if args.len() < 3 {
                eprintln!("Usage: cargo xtask update-deps <version>");
                exit(1);
            }
            update_deps(&args[2])
        }
        "publish" => {
            if args.len() < 3 {
                eprintln!("Usage: cargo xtask publish <pkg_dir> [registry_url]");
                exit(1);
            }
            let reg = args.get(3).map(|s| s.as_str());
            publish(&args[2], reg)
        }
        "--help" | "-h" | "help" => {
            println!("Cargo xtask - Automation tasks for dockex");
            println!("\nUsage:");
            println!("  cargo xtask prepare-platform <platform> <arch> <version> <binary_path>");
            println!("  cargo xtask update-deps <version>");
            println!("  cargo xtask publish <pkg_dir> [registry_url]");
            exit(0);
        }
        _ => {
            eprintln!("Unknown command: {command}");
            eprintln!("Available commands: prepare-platform, update-deps, publish");
            exit(1);
        }
    };

    if let Err(e) = result {
        eprintln!("❌ Error: {e}");
        exit(1);
    }
}
