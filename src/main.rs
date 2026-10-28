mod compose;
mod config;
mod prompt;
mod redeploy;

use std::env;
use std::process::exit;

fn parse_engine_flag(args: &[String]) -> Option<String> {
    if let Some(pos) = args.iter().position(|a| a == "--engine") {
        if let Some(next_val) = args.get(pos + 1) {
            let val = next_val.to_lowercase();
            if val == "docker" || val == "podman" {
                return Some(val);
            }
        }
    }

    for arg in args {
        if let Some(val) = arg.strip_prefix("--engine=") {
            let lower = val.to_lowercase();
            if lower == "docker" || lower == "podman" {
                return Some(lower);
            }
        }
    }

    if args.iter().any(|a| a == "--docker") {
        return Some("docker".to_string());
    }

    if args.iter().any(|a| a == "--podman") {
        return Some("podman".to_string());
    }

    None
}

fn print_help() {
    println!(
        r#"
Dockex CLI - Multi-project Docker & Podman deployment & memory manager

Usage:
  dockex                         Run the full 7-step redeploy workflow (auto-detects engine or prompts)
  dockex redeploy                Alias for redeploy workflow
  dockex --engine <engine>       Run redeployment with specific engine (docker | podman)
  dockex --skip-flush, -s        Skip kernel memory cache drop (Step 3 & Step 5)
  dockex config set <key> <val>  Store custom configuration / API key
  dockex config get <key>        Retrieve stored configuration
  dockex --help                  Show this help message

Engine Resolution Priority:
  1. CLI Flag (--engine, --docker, --podman)
  2. Compose extension attribute (x-engine: "docker" | "podman")
  3. Interactive Prompt (defaults to stored config)
"#
    );
}

fn main() {
    let args: Vec<String> = env::args().skip(1).collect();
    let raw_command = args.first().map(|s| s.as_str()).unwrap_or("redeploy");
    let explicit_engine = parse_engine_flag(&args);
    let skip_flush = args
        .iter()
        .any(|a| a == "--skip-flush" || a == "--no-flush" || a == "-s");

    // If command is a flag like --engine, --docker, or --podman, treat as redeploy
    let command = if raw_command.starts_with("--engine")
        || raw_command == "--docker"
        || raw_command == "--podman"
        || raw_command == "--skip-flush"
        || raw_command == "--no-flush"
        || raw_command == "-s"
    {
        "redeploy"
    } else {
        raw_command
    };

    match command {
        "redeploy" | "deploy" => {
            let loaded_config = config::load_config();

            let should_skip_flush = skip_flush
                || loaded_config
                    .get("flushMemory")
                    .and_then(|v| v.as_bool())
                    .map(|b| !b)
                    .unwrap_or(false);

            redeploy::execute_redeploy(explicit_engine.as_deref(), should_skip_flush);
        }

        "config" => {
            let sub_action = args.get(1).map(|s| s.as_str());
            let key = args.get(2).map(|s| s.as_str());
            let value = args.get(3).map(|s| s.as_str());
            let mut loaded_config = config::load_config();

            if sub_action == Some("set") && key.is_some() && value.is_some() {
                let k = key.unwrap();
                let v = value.unwrap();

                let json_val = if v.eq_ignore_ascii_case("true") {
                    serde_json::Value::Bool(true)
                } else if v.eq_ignore_ascii_case("false") {
                    serde_json::Value::Bool(false)
                } else {
                    serde_json::Value::String(v.to_string())
                };

                loaded_config.insert(k.to_string(), json_val);
                if let Err(e) = config::save_config(&loaded_config) {
                    eprintln!("❌ Failed to save config: {e}");
                    exit(1);
                }
                println!("✅ Config updated: {k} = {v}");
            } else if sub_action == Some("get") && key.is_some() {
                let k = key.unwrap();
                match loaded_config.get(k) {
                    Some(serde_json::Value::String(s)) => println!("{k}: {s}"),
                    Some(other) => println!("{k}: {other}"),
                    None => println!("{k}: (not set)"),
                }
            } else {
                println!("Usage: dockex config set <key> <value>");
                println!("       dockex config get <key>");
            }
        }

        "--help" | "-h" | "help" => {
            print_help();
        }

        unknown => {
            eprintln!(
                "Unknown command: \"{unknown}\". Run \"dockex --help\" for available commands."
            );
            exit(1);
        }
    }
}
