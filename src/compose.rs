use std::fs;
use std::path::{Path, PathBuf};

pub const COMPOSE_FILES: [&str; 4] = [
    "compose.yaml",
    "compose.yml",
    "docker-compose.yaml",
    "docker-compose.yml",
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServiceItem {
    pub name: String,
    pub image: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComposeMetadata {
    pub file_name: String,
    pub file_path: PathBuf,
    pub engine: Option<String>,
    pub services: Vec<ServiceItem>,
}

pub fn find_compose_file(cwd: &Path) -> Option<PathBuf> {
    COMPOSE_FILES
        .iter()
        .map(|file| cwd.join(file))
        .find(|path| path.exists())
}

fn clean_yaml_val(val: &str) -> String {
    // Remove comments
    let val_without_comment = match val.split_once('#') {
        Some((before, _)) => before,
        None => val,
    };
    val_without_comment
        .trim()
        .trim_matches(|c| c == '\'' || c == '"')
        .to_string()
}

pub fn parse_compose_content(
    file_name: &str,
    file_path: PathBuf,
    content: &str,
) -> ComposeMetadata {
    let mut engine: Option<String> = None;
    let mut services: Vec<ServiceItem> = Vec::new();

    let mut in_services = false;
    let mut service_indent: Option<usize> = None;
    let mut current_service: Option<ServiceItem> = None;

    for line in content.lines() {
        // Strip trailing CR/LF and ignore pure comments / blank lines
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }

        // Count leading spaces to determine YAML hierarchy
        let indent = line.chars().take_while(|&c| c == ' ').count();

        // Check for x-engine at root level
        if indent == 0 && trimmed.starts_with("x-engine:") {
            let raw_val = &trimmed["x-engine:".len()..];
            let clean = clean_yaml_val(raw_val).to_lowercase();
            if clean == "docker" || clean == "podman" {
                engine = Some(clean);
            }
            continue;
        }

        if indent == 0 {
            // Check if entering services block
            if trimmed == "services:" || trimmed.starts_with("services:") {
                in_services = true;
                service_indent = None;
                continue;
            } else if in_services {
                // Encountered another top-level key (e.g. volumes:, networks:, configs:)
                if let Some(s) = current_service.take() {
                    services.push(s);
                }
                in_services = false;
                service_indent = None;
                continue;
            }
        }

        if in_services {
            match service_indent {
                None => {
                    // First service under services:
                    if indent > 0 && trimmed.ends_with(':') {
                        service_indent = Some(indent);
                        let s_name = trimmed.trim_end_matches(':').trim().to_string();
                        current_service = Some(ServiceItem {
                            name: s_name,
                            image: "[local build]".to_string(),
                        });
                    }
                }
                Some(base_indent) => {
                    if indent == base_indent {
                        // Transition to next service definition
                        if trimmed.ends_with(':') {
                            if let Some(s) = current_service.take() {
                                services.push(s);
                            }
                            let s_name = trimmed.trim_end_matches(':').trim().to_string();
                            current_service = Some(ServiceItem {
                                name: s_name,
                                image: "[local build]".to_string(),
                            });
                        }
                    } else if indent > base_indent {
                        // Properties inside current service
                        if let Some(ref mut s) = current_service {
                            if trimmed.starts_with("image:") {
                                let raw_val = &trimmed["image:".len()..];
                                let clean = clean_yaml_val(raw_val);
                                if !clean.is_empty() {
                                    s.image = clean;
                                }
                            } else if trimmed.starts_with("build:") {
                                // If image isn't explicitly set to an image tag, keep it as [local build]
                                if s.image.is_empty() {
                                    s.image = "[local build]".to_string();
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    if let Some(s) = current_service.take() {
        services.push(s);
    }

    ComposeMetadata {
        file_name: file_name.to_string(),
        file_path,
        engine,
        services,
    }
}

pub fn inspect_compose(cwd: &Path) -> Option<ComposeMetadata> {
    let path = find_compose_file(cwd)?;
    let file_name = path.file_name()?.to_string_lossy().to_string();
    let content = fs::read_to_string(&path).ok()?;
    Some(parse_compose_content(&file_name, path, &content))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_x_engine_and_services() {
        let yaml = r#"
version: '3.8'
x-engine: "podman"

services:
  web:
    image: ghcr.io/sinkaroid/web:latest
    ports:
      - "80:80"

  worker:
    build: .
    environment:
      - DEBUG=1

  database:
    image: 'postgres:15-alpine'
    volumes:
      - pgdata:/var/lib/postgresql/data

volumes:
  pgdata:
"#;
        let meta = parse_compose_content("compose.yaml", PathBuf::from("compose.yaml"), yaml);
        assert_eq!(meta.engine, Some("podman".to_string()));
        assert_eq!(meta.services.len(), 3);
        assert_eq!(meta.services[0].name, "web");
        assert_eq!(meta.services[0].image, "ghcr.io/sinkaroid/web:latest");
        assert_eq!(meta.services[1].name, "worker");
        assert_eq!(meta.services[1].image, "[local build]");
        assert_eq!(meta.services[2].name, "database");
        assert_eq!(meta.services[2].image, "postgres:15-alpine");
    }

    #[test]
    fn test_parse_no_x_engine() {
        let yaml = r#"
services:
  api:
    image: my-registry/api:v1
"#;
        let meta = parse_compose_content(
            "docker-compose.yml",
            PathBuf::from("docker-compose.yml"),
            yaml,
        );
        assert_eq!(meta.engine, None);
        assert_eq!(meta.services.len(), 1);
        assert_eq!(meta.services[0].name, "api");
        assert_eq!(meta.services[0].image, "my-registry/api:v1");
    }
}
