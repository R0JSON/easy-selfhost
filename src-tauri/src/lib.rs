use std::process::Command;
use std::fs;
use tauri::{AppHandle, Manager};
use serde::{Deserialize, Serialize};

#[derive(Deserialize)]
struct DeployConfig {
    target_ip: String,
    target_user: String,
    hostname: String,
    ssh_key: String,
    target_device: String,
    nextcloud_hostname: String,
    admin_password: String,
    ssl_enable: bool,
    acme_email: String,
}

#[derive(Serialize)]
struct DeployResult {
    success: bool,
    message: String,
}

#[tauri::command]
async fn deploy(app: AppHandle, config: DeployConfig) -> Result<DeployResult, String> {
    let app_dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
    let deploy_dir = app_dir.join("deploy");
    fs::create_dir_all(&deploy_dir).map_err(|e| e.to_string())?;

    // Generate disk-config.nix
    let disk_config = include_str!("../nix/disk-config.nix")
        .replace("{{ target_device }}", &config.target_device);
    fs::write(deploy_dir.join("disk-config.nix"), disk_config).map_err(|e| e.to_string())?;

    // Generate configuration.nix
    let ssl_enable_str = if config.ssl_enable { "true" } else { "false" };
    let acme_config = if config.ssl_enable {
        format!(
            "security.acme = {{ acceptTerms = true; defaults.email = \"{}\"; }};",
            config.acme_email
        )
    } else {
        "".to_string()
    };

    let configuration = include_str!("../nix/configuration.nix")
        .replace("{{ hostname }}", &config.hostname)
        .replace("{{ ssh_key }}", &config.ssh_key)
        .replace("{{ nextcloud_hostname }}", &config.nextcloud_hostname)
        .replace("{{ ssl_enable }}", ssl_enable_str)
        .replace("{{ acme_config }}", &acme_config);
    fs::write(deploy_dir.join("configuration.nix"), configuration).map_err(|e| e.to_string())?;

    // Generate flake.nix
    let flake = include_str!("../nix/flake.nix")
        .replace("{{ hostname }}", &config.hostname);
    fs::write(deploy_dir.join("flake.nix"), flake).map_err(|e| e.to_string())?;

    // Generate admin password file for extra-files
    let extra_files_dir = deploy_dir.join("extra-files/etc");
    fs::create_dir_all(&extra_files_dir).map_err(|e| e.to_string())?;
    fs::write(extra_files_dir.join("nextcloud-admin-pass"), &config.admin_password).map_err(|e| e.to_string())?;

    // Run nixos-anywhere
    let output = Command::new("nixos-anywhere")
        .arg("--flake")
        .arg(format!(".#{}", config.hostname))
        .arg("--extra-files")
        .arg("extra-files")
        .arg(format!("{}@{}", config.target_user, config.target_ip))
        .current_dir(&deploy_dir)
        .output()
        .map_err(|e| format!("Failed to execute nixos-anywhere: {}", e))?;

    if output.status.success() {
        Ok(DeployResult {
            success: true,
            message: "Deployment successful!".to_string(),
        })
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        Ok(DeployResult {
            success: false,
            message: format!("Deployment failed: {}", stderr),
        })
    }
}

#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Rust!", name)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![greet, deploy])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
