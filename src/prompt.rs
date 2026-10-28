use std::io::{self, Write};

pub fn select_runtime_stack(default_engine: &str) -> String {
    let default = if default_engine.eq_ignore_ascii_case("podman") {
        "podman"
    } else {
        "docker"
    };

    println!("⚙️  Select container runtime stack:");
    println!(
        "  1) Docker{}",
        if default == "docker" { " (default)" } else { "" }
    );
    println!(
        "  2) Podman{}\n",
        if default == "podman" { " (default)" } else { "" }
    );

    let default_choice = if default == "docker" { "1" } else { "2" };
    print!("Enter choice [1/2 or docker/podman] (default: {default_choice}): ");
    let _ = io::stdout().flush();

    let mut input = String::new();
    if io::stdin().read_line(&mut input).is_ok() {
        let trimmed = input.trim().to_lowercase();
        if trimmed.is_empty() {
            return default.to_string();
        }
        if trimmed == "1" || trimmed == "docker" {
            return "docker".to_string();
        }
        if trimmed == "2" || trimmed == "podman" {
            return "podman".to_string();
        }
        println!("⚠️  Unrecognized selection \"{trimmed}\", defaulting to {default}.");
    }

    default.to_string()
}
