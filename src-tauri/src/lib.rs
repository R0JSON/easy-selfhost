use std::process::Command;
use std::fs;
use std::path::PathBuf;
use tauri::{AppHandle, Manager};
use serde::{Deserialize, Serialize};

#[derive(Deserialize)]
struct DeployConfig {
    // Connection
    target_ip: String,
    target_user: String,

    // System
    hostname: String,
    ssh_key: String,
    target_device: String,

    // Nextcloud (always enabled)
    nextcloud_hostname: String,
    admin_password: String,
    ssl_enable: bool,
    acme_email: String,

    // SSH identity
    ssh_identity_file: Option<String>,

    // Jellyfin
    jellyfin_enable: bool,
    jellyfin_hostname: Option<String>,
    jellyfin_media_dir: Option<String>,
    jellyfin_open_firewall: Option<bool>,

    // Vaultwarden
    vaultwarden_enable: bool,
    vaultwarden_hostname: Option<String>,
    vaultwarden_port: Option<u16>,
    vaultwarden_admin_token: Option<String>,
    vaultwarden_signups_allowed: Option<bool>,
}

#[derive(Deserialize)]
struct ExistingDeployConfig {
    flake_dir: String,
    target_ip: String,
    target_user: String,
    ssh_identity_file: Option<String>,
    admin_password: Option<String>, // optional for existing configs
}

#[derive(Serialize)]
struct DeployResult {
    success: bool,
    message: String,
}

#[derive(Serialize)]
struct SshKeyResult {
    public_key: String,
    private_key_path: String,
}

#[derive(Serialize)]
struct DependenciesResult {
    nix: bool,
    ssh: bool,
}

#[tauri::command]
async fn check_dependencies() -> DependenciesResult {
    let nix = Command::new("nix").arg("--version").output().is_ok();
    let ssh = Command::new("ssh").arg("-V").output().is_ok();

    DependenciesResult {
        nix,
        ssh,
    }
}

#[tauri::command]
async fn generate_ssh_key(app: tauri::AppHandle) -> Result<SshKeyResult, String> {
    let app_dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
    let ssh_dir = app_dir.join("ssh");
    fs::create_dir_all(&ssh_dir).map_err(|e| e.to_string())?;

    let key_path = ssh_dir.join("id_ed25519");
    
    // Remove existing keys if they exist to avoid prompts
    if key_path.exists() {
        fs::remove_file(&key_path).ok();
        fs::remove_file(ssh_dir.join("id_ed25519.pub")).ok();
    }

    let output = Command::new("ssh-keygen")
        .arg("-t")
        .arg("ed25519")
        .arg("-f")
        .arg(&key_path)
        .arg("-N")
        .arg("") // empty passphrase
        .output()
        .map_err(|e| format!("Failed to run ssh-keygen: {}", e))?;

    if !output.status.success() {
        return Err(format!("ssh-keygen failed: {}", String::from_utf8_lossy(&output.stderr)));
    }

    let public_key = fs::read_to_string(ssh_dir.join("id_ed25519.pub"))
        .map_err(|e| format!("Failed to read public key: {}", e))?;

    Ok(SshKeyResult {
        public_key: public_key.trim().to_string(),
        private_key_path: key_path.to_string_lossy().to_string(),
    })
}

fn generate_nix_files(deploy_dir: &PathBuf, config: &DeployConfig) -> Result<(), String> {
    fs::create_dir_all(deploy_dir).map_err(|e| e.to_string())?;

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

    // Jellyfin block
    let jellyfin_block = if config.jellyfin_enable {
        let hostname = config.jellyfin_hostname.as_deref().unwrap_or("");
        let media_dir = config.jellyfin_media_dir.as_deref().unwrap_or("");
        let open_fw = config.jellyfin_open_firewall.unwrap_or(false);
        format!(
            "services.jellyfin = {{ enable = true; openFirewall = {}; dataDir = \"{}\"; hostName = \"{}\"; }};",
            if open_fw { "true" } else { "false" },
            media_dir,
            hostname
        )
    } else {
        "".to_string()
    };

    // Vaultwarden block
    let vaultwarden_enable = if config.vaultwarden_enable {"true"} else {"false"};
    let vaultwarden_hostname = config.vaultwarden_hostname.as_deref().unwrap_or("");
    let vaultwarden_port = config.vaultwarden_port.map(|p| p.to_string()).unwrap_or_else(|| "80".to_string());
    let vaultwarden_admin_token = config.vaultwarden_admin_token.as_deref().unwrap_or("");
    let vaultwarden_signups = if config.vaultwarden_signups_allowed.unwrap_or(false) {"true"} else {"false"};
    

    let configuration = include_str!("../nix/configuration.nix")
        .replace("{{ hostname }}", &config.hostname)
        .replace("{{ ssh_key }}", &config.ssh_key)
        .replace("{{ nextcloud_hostname }}", &config.nextcloud_hostname)
        .replace("{{ ssl_enable }}", ssl_enable_str)
        .replace("{{ acme_config }}", &acme_config)
        .replace("{{ jellyfin_block }}", &jellyfin_block)
        .replace("{{ vaultwarden_enable }}", vaultwarden_enable)
        .replace("{{ vaultwarden_hostname }}", vaultwarden_hostname)
        .replace("{{ vaultwarden_signups }}", vaultwarden_signups)
        .replace("{{ vaultwarden_port }}", &vaultwarden_port)
        ;
    fs::write(deploy_dir.join("configuration.nix"), configuration).map_err(|e| e.to_string())?;

    // Generate flake.nix
    let flake = include_str!("../nix/flake.nix")
        .replace("{{ hostname }}", &config.hostname);
    fs::write(deploy_dir.join("flake.nix"), flake).map_err(|e| e.to_string())?;

    Ok(())
}

#[tauri::command]
async fn save_configuration(_app: tauri::AppHandle, config: DeployConfig, save_path: String) -> Result<DeployResult, String> {
    let deploy_dir = PathBuf::from(save_path);
    generate_nix_files(&deploy_dir, &config)?;

    Ok(DeployResult {
        success: true,
        message: format!("Configuration saved to {:?}", deploy_dir),
    })
}

#[tauri::command]
async fn deploy(app: tauri::AppHandle, config: DeployConfig) -> Result<DeployResult, String> {
    let app_dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
    let deploy_dir = app_dir.join("deploy");
    
    generate_nix_files(&deploy_dir, &config)?;

    // Generate admin password file for extra-files
    let extra_files_dir = deploy_dir.join("extra-files/etc");
    fs::create_dir_all(&extra_files_dir).map_err(|e| e.to_string())?;
    fs::write(extra_files_dir.join("nextcloud-admin-pass"), &config.admin_password).map_err(|e| e.to_string())?;

    // Run nixos-anywhere
    let mut cmd = Command::new("pkexec");
    cmd.arg("nix")
       .arg("--extra-experimental-features")
       .arg("nix-command flakes")
       .arg("run")
       .arg("github:nix-community/nixos-anywhere")
       .arg("--")
       .arg("--flake")
       .arg(format!(".#{}", config.hostname))
       .arg("--extra-files")
       .arg("extra-files");

    if let Some(ref identity) = config.ssh_identity_file {
        cmd.arg("-i").arg(identity);
    }

    cmd.arg("--target-host")
       .arg(format!("{}@{}", config.target_user, config.target_ip));

    let output = cmd
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
        let stdout = String::from_utf8_lossy(&output.stdout);
        Ok(DeployResult {
            success: false,
            message: format!("Deployment failed:\n{}\n{}", stderr, stdout),
        })
    }
}

#[tauri::command]
async fn deploy_existing(config: ExistingDeployConfig) -> Result<DeployResult, String> {
    let deploy_dir = PathBuf::from(config.flake_dir);
    let mut cmd = Command::new("pkexec");
    cmd.arg("nix")
       .arg("--extra-experimental-features")
       .arg("nix-command flakes")
       .arg("run")
       .arg("github:nix-community/nixos-anywhere")
       .arg("--")
       .arg("--flake")
       .arg(".#"); // Assumes default nixosConfigurations in the flake

    // Optional extra-files if admin_password was provided
    if let Some(pwd) = config.admin_password {
        let extra_files_dir = deploy_dir.join("extra-files/etc");
        fs::create_dir_all(&extra_files_dir).map_err(|e| e.to_string())?;
        fs::write(extra_files_dir.join("nextcloud-admin-pass"), pwd).map_err(|e| e.to_string())?;
        cmd.arg("--extra-files").arg("extra-files");
    }

    if let Some(ref identity) = config.ssh_identity_file {
        cmd.arg("-i").arg(identity);
    }

    cmd.arg("--target-host")
       .arg(format!("{}@{}", config.target_user, config.target_ip));

    let output = cmd
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
        let stdout = String::from_utf8_lossy(&output.stdout);
        Ok(DeployResult {
            success: false,
            message: format!("Deployment failed:\n{}\n{}", stderr, stdout),
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
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            greet, 
            deploy, 
            generate_ssh_key, 
            check_dependencies, 
            save_configuration, 
            deploy_existing
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
