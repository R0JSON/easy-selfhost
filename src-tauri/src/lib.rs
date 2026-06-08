use std::process::{Command, Stdio};
use std::io::{BufRead, BufReader};
use std::thread;
use std::fs;
use std::path::PathBuf;
use tauri::{Manager, Emitter};
use serde::{Deserialize, Serialize};

fn system_command(cmd: &str) -> Command {
    let mut c = Command::new(cmd);
    // Remove LD_LIBRARY_PATH to prevent AppImage bundled libraries
    // from interfering with system binaries (e.g., nix, ssh).
    c.env_remove("LD_LIBRARY_PATH");
    c
}

#[derive(Deserialize)]
struct DeployConfig {
    // Connection
    target_ip: String,
    target_user: String,

    // System
    hostname: String,
    ssh_key: String,
    target_device: String,
    ssl_enable: bool,
    acme_email: Option<String>,

    // Nextcloud
    nextcloud_enable: bool,
    nextcloud_hostname: Option<String>,
    admin_password: Option<String>,

    // SSH identity
    ssh_identity_file: Option<String>,
    ssh_password: Option<String>,

    // Jellyfin
    jellyfin_enable: bool,
    jellyfin_hostname: Option<String>,
    jellyfin_media_dir: Option<String>,

    // Vaultwarden
    vaultwarden_enable: bool,
    vaultwarden_hostname: Option<String>,
    vaultwarden_admin_token: Option<String>,
    vaultwarden_signups_allowed: Option<bool>,

    // Immich
    immich_enable: bool,
    immich_hostname: Option<String>,

    // Gitea
    gitea_enable: bool,
    gitea_hostname: Option<String>,

    // Uptime Kuma
    uptime_kuma_enable: bool,
    uptime_kuma_hostname: Option<String>,

    // Vikunja
    vikunja_enable: bool,
    vikunja_hostname: Option<String>,

    // Tailscale
    tailscale_enable: bool,

    // AdGuard Home
    adguardhome_enable: bool,
    adguardhome_hostname: Option<String>,
}

#[derive(Deserialize)]
struct ExistingDeployConfig {
    flake_dir: String,
    target_ip: String,
    target_user: String,
    ssh_identity_file: Option<String>,
    ssh_password: Option<String>,
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
    sshpass: bool,
    cpio: bool,
    git: bool,
}

#[tauri::command]
async fn check_dependencies() -> DependenciesResult {
    let nix = system_command("nix").arg("--version").output().is_ok();
    let ssh = system_command("ssh").arg("-V").output().is_ok();
    let sshpass = system_command("sshpass").arg("-V").output().is_ok();
    let cpio = system_command("cpio").arg("--version").output().is_ok();
    let git = system_command("git").arg("--version").output().is_ok();

    DependenciesResult {
        nix,
        ssh,
        sshpass,
        cpio,
        git,
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

    let output = system_command("ssh-keygen")
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
            config.acme_email.as_deref().unwrap_or("")
        )
    } else {
        "".to_string()
    };

    // Nextcloud block
    let nextcloud_block = if config.nextcloud_enable {
        let hostname = config.nextcloud_hostname.as_deref().unwrap_or("");
        format!(r#"
  services.nextcloud = {{
    enable = true;
    hostName = "{}";
    package = pkgs.nextcloud29;
    
    database.createLocally = true;
    configureRedis = true;
    
    config = {{
      dbtype = "pgsql";
      adminuser = "admin";
      adminpassFile = "/etc/nextcloud-admin-pass";
    }};
  }};

  systemd.tmpfiles.rules = [
    "z /etc/nextcloud-admin-pass 0600 nextcloud nextcloud - -"
  ];"#, hostname)
    } else {
        "".to_string()
    };

    let nextcloud_nginx_vhost = if config.nextcloud_enable {
        let hostname = config.nextcloud_hostname.as_deref().unwrap_or("");
        format!(r#"
  services.nginx.virtualHosts."{}" = {{
    forceSSL = {};
    enableACME = {};
  }};"#, hostname, ssl_enable_str, ssl_enable_str)
    } else { "".to_string() };

    // Jellyfin block
    let jellyfin_block = if config.jellyfin_enable {
        let media_dir = config.jellyfin_media_dir.as_deref().unwrap_or("");
        format!(
            "services.jellyfin = {{ enable = true; dataDir = \"{}\"; }};",
            media_dir
        )
    } else {
        "".to_string()
    };

    let jellyfin_nginx_vhost = if config.jellyfin_enable {
        let hostname = config.jellyfin_hostname.as_deref().unwrap_or("");
        format!(r#"
  services.nginx.virtualHosts."{}" = {{
    forceSSL = {};
    enableACME = {};
    locations."/" = {{
      proxyPass = "http://127.0.0.1:8096";
      proxyWebsockets = true;
    }};
  }};"#, hostname, ssl_enable_str, ssl_enable_str)
    } else { "".to_string() };

    // Vaultwarden block
    let vaultwarden_enable = if config.vaultwarden_enable {"true"} else {"false"};
    let vaultwarden_hostname = config.vaultwarden_hostname.as_deref().unwrap_or("");
    let vaultwarden_env_file = if config.vaultwarden_admin_token.is_some() {
        "environmentFile = \"/etc/vaultwarden.env\";"
    } else {
        ""
    };
    let vaultwarden_signups = if config.vaultwarden_signups_allowed.unwrap_or(false) {"true"} else {"false"};
    
    let vaultwarden_nginx_vhost = if config.vaultwarden_enable {
        format!(r#"
  services.nginx.virtualHosts."{}" = {{
    forceSSL = {};
    enableACME = {};
    locations."/" = {{
      proxyPass = "http://127.0.0.1:8222";
      proxyWebsockets = true;
    }};
  }};"#, vaultwarden_hostname, ssl_enable_str, ssl_enable_str)
    } else { "".to_string() };

    // --- IMMICH ---

    let immich_block = if config.immich_enable {
        "services.immich = { enable = true; port = 2283; host = \"127.0.0.1\"; };".to_string()
    } else { "".to_string() };

    let immich_nginx_vhost = if config.immich_enable {
        let hostname = config.immich_hostname.as_deref().unwrap_or("");
        format!(r#"
  services.nginx.virtualHosts."{}" = {{
    forceSSL = {};
    enableACME = {};
    locations."/" = {{ proxyPass = "http://127.0.0.1:2283"; proxyWebsockets = true; }};
  }};"#, hostname, ssl_enable_str, ssl_enable_str)
    } else { "".to_string() };

    // --- GITEA ---
    let gitea_block = if config.gitea_enable {
        r#"services.gitea = { enable = true; appName = "Mój Prywatny Skarbiec Kodu"; database.type = "sqlite3"; settings.server.HTTP_PORT = 8080; };"#.to_string()
    } else { "".to_string() };

    let gitea_nginx_vhost = if config.gitea_enable {
        let hostname = config.gitea_hostname.as_deref().unwrap_or("");
        format!(r#"
  services.nginx.virtualHosts."{}" = {{
    forceSSL = {};
    enableACME = {};
    locations."/" = {{ proxyPass = "http://127.0.0.1:8080"; proxyWebsockets = true; }};
  }};"#, hostname, ssl_enable_str, ssl_enable_str)
    } else { "".to_string() };

    // --- UPTIME KUMA ---
    let uptime_kuma_block = if config.uptime_kuma_enable {
        r#"services.uptime-kuma = { enable = true; settings = { HOST = "127.0.0.1"; PORT = "3001"; }; };"#.to_string()
    } else { "".to_string() };

    let uptime_kuma_nginx_vhost = if config.uptime_kuma_enable {
        let hostname = config.uptime_kuma_hostname.as_deref().unwrap_or("");
        format!(r#"
  services.nginx.virtualHosts."{}" = {{
    forceSSL = {};
    enableACME = {};
    locations."/" = {{ proxyPass = "http://127.0.0.1:3001"; proxyWebsockets = true; }};
  }};"#, hostname, ssl_enable_str, ssl_enable_str)
    } else { "".to_string() };

    // --- VIKUNJA ---
    let vikunja_block = if config.vikunja_enable {
        let hostname = config.vikunja_hostname.as_deref().unwrap_or("vikunja.local");
        format!(r#"services.vikunja = {{ enable = true; frontendScheme = "https"; frontendHostname = "{}"; port = 3456; settings.service.frontendurl = "https://{}/"; }};"#, hostname, hostname)
    } else { "".to_string() };

    let vikunja_nginx_vhost = if config.vikunja_enable {
        let hostname = config.vikunja_hostname.as_deref().unwrap_or("");
        format!(r#"
  services.nginx.virtualHosts."{}" = {{
    forceSSL = {};
    enableACME = {};
    locations."/" = {{ proxyPass = "http://127.0.0.1:3456"; proxyWebsockets = true; }};
  }};"#, hostname, ssl_enable_str, ssl_enable_str)
    } else { "".to_string() };

    // --- TAILSCALE ---
    let tailscale_block = if config.tailscale_enable {
        "services.tailscale.enable = true;".to_string()
    } else { "".to_string() };

    // --- ADGUARD HOME ---
    let adguardhome_block = if config.adguardhome_enable {
        "services.adguardhome = { enable = true; openFirewall = true; };".to_string()
    } else { "".to_string() };

    let adguardhome_nginx_vhost = if config.adguardhome_enable {
        let hostname = config.adguardhome_hostname.as_deref().unwrap_or("");
        format!(r#"
  services.nginx.virtualHosts."{}" = {{
    forceSSL = {};
    enableACME = {};
    locations."/" = {{ proxyPass = "http://127.0.0.1:3000"; proxyWebsockets = true; }};
  }};"#, hostname, ssl_enable_str, ssl_enable_str)
    } else { "".to_string() };

    // --- COMPILE configuration.nix ---
    let configuration = include_str!("../nix/configuration.nix")
        .replace("{{ hostname }}", &config.hostname)
        .replace("{{ ssh_key }}", &config.ssh_key)
        .replace("{{ ssl_enable }}", ssl_enable_str)
        .replace("{{ acme_config }}", &acme_config)
        .replace("{{ nextcloud_block }}", &nextcloud_block)
        .replace("{{ nextcloud_nginx_vhost }}", &nextcloud_nginx_vhost)
        .replace("{{ jellyfin_block }}", &jellyfin_block)
        .replace("{{ jellyfin_nginx_vhost }}", &jellyfin_nginx_vhost)
        .replace("{{ vaultwarden_enable }}", vaultwarden_enable)
        .replace("{{ vaultwarden_env_file }}", vaultwarden_env_file)
        .replace("{{ vaultwarden_hostname }}", vaultwarden_hostname)
        .replace("{{ vaultwarden_signups }}", vaultwarden_signups)
        .replace("{{ vaultwarden_nginx_vhost }}", &vaultwarden_nginx_vhost)
        .replace("{{ immich_block }}", &immich_block)
        .replace("{{ immich_nginx_vhost }}", &immich_nginx_vhost)
        .replace("{{ gitea_block }}", &gitea_block)
        .replace("{{ gitea_nginx_vhost }}", &gitea_nginx_vhost)
        .replace("{{ uptime_kuma_block }}", &uptime_kuma_block)
        .replace("{{ uptime_kuma_nginx_vhost }}", &uptime_kuma_nginx_vhost)
        .replace("{{ vikunja_block }}", &vikunja_block)
        .replace("{{ vikunja_nginx_vhost }}", &vikunja_nginx_vhost)
        .replace("{{ tailscale_block }}", &tailscale_block)
        .replace("{{ adguardhome_block }}", &adguardhome_block)
        .replace("{{ adguardhome_nginx_vhost }}", &adguardhome_nginx_vhost);

    fs::write(deploy_dir.join("configuration.nix"), configuration).map_err(|e| e.to_string())?;

    // Generate flake.nix
    let flake = include_str!("../nix/flake.nix")
        .replace("{{ hostname }}", &config.hostname);
    fs::write(deploy_dir.join("flake.nix"), flake).map_err(|e| e.to_string())?;

    // Generate secret files for extra-files
    let extra_files_dir = deploy_dir.join("extra-files/etc");

    if config.admin_password.is_some() || config.vaultwarden_admin_token.is_some() {
        fs::create_dir_all(&extra_files_dir).map_err(|e| e.to_string())?;
    }

    if let Some(pwd) = &config.admin_password {
        fs::write(extra_files_dir.join("nextcloud-admin-pass"), pwd).map_err(|e| e.to_string())?;
    }

    if let Some(token) = &config.vaultwarden_admin_token {
        fs::write(extra_files_dir.join("vaultwarden.env"), format!("ADMIN_TOKEN={}", token)).map_err(|e| e.to_string())?;
    }

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

async fn run_ssh_copy_id(
    target_user: &str,
    target_ip: &str,
    ssh_password: Option<&str>,
    ssh_identity_file: Option<&str>,
    app: &tauri::AppHandle,
) -> Result<(), String> {
    if let Some(password) = ssh_password {
        let path_env = std::env::var("PATH").unwrap_or_else(|_| "".to_string());
        let combined_path = format!("{}:/run/current-system/sw/bin:/nix/var/nix/profiles/default/bin:/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin", path_env);

        let _ = app.emit("deploy-progress", "SSH password provided. Attempting to install public key via ssh-copy-id...");

        if let Ok(home) = std::env::var("HOME") {
            let user_ssh_dir = std::path::PathBuf::from(&home).join(".ssh");
            if !user_ssh_dir.exists() {
                let _ = app.emit("deploy-progress", format!("Creating {} (required by ssh-copy-id)...", user_ssh_dir.display()));
                if let Err(e) = fs::create_dir_all(&user_ssh_dir) {
                    return Err(format!("Failed to create {}: {}", user_ssh_dir.display(), e));
                }
                // chmod 700 — required by ssh-copy-id / openssh
                #[cfg(unix)]
                {
                    use std::os::unix::fs::PermissionsExt;
                    let _ = fs::set_permissions(&user_ssh_dir, fs::Permissions::from_mode(0o700));
                }
            }
        }

        // 1. Copy SSH Key
        let mut cmd = system_command("sshpass");
        cmd.env("PATH", &combined_path)
            .env("SSHPASS", password)
            .arg("-e")
            .arg("ssh-copy-id")
            .arg("-o")
            .arg("StrictHostKeyChecking=accept-new")
            .arg("-o")
            .arg("IdentitiesOnly=yes")
            .arg("-o")
            .arg("PreferredAuthentications=password");

        if let Some(identity) = ssh_identity_file {
            cmd.arg("-i").arg(identity);
        }

        cmd.arg(format!("{}@{}", target_user, target_ip));

        let output = cmd.output().map_err(|e| format!("Failed to execute sshpass ssh-copy-id: {}", e))?;
        if !output.status.success() {
            // Ignore failures if the key already exists
            let stderr = String::from_utf8_lossy(&output.stderr);
            if !stderr.contains("already exist") {
                let _ = app.emit("deploy-progress", format!("ssh-copy-id warning: {}", stderr));
                return Err(format!(
                    "ssh-copy-id failed: {}\n{}",
                    stderr,
                    String::from_utf8_lossy(&output.stdout)
                ));
            } else {
                let _ = app.emit("deploy-progress", "SSH key already exists on the remote system.");
            }
        } else {
            let _ = app.emit("deploy-progress", "SSH key successfully installed.");
        }

        // 2. Automatically configure passwordless sudo and copy SSH keys to root for kexec
        if target_user != "root" {
            let _ = app.emit("deploy-progress", format!("Configuring passwordless sudo and copying SSH keys to root for user '{}'...", target_user));
            let sudo_cmd = format!(
                "echo '{pwd}' | sudo -S sh -c 'echo \"{user} ALL=(ALL) NOPASSWD: ALL\" > /etc/sudoers.d/{user} && mkdir -p /root/.ssh && cat ~{user}/.ssh/authorized_keys >> /root/.ssh/authorized_keys && chmod 600 /root/.ssh/authorized_keys'",
                pwd=password, user=target_user
            );

            let mut sudo_setup = system_command("sshpass");
            sudo_setup.env("PATH", combined_path)
                .env("SSHPASS", password)
                .arg("-e")
                .arg("ssh")
                .arg("-o")
                .arg("StrictHostKeyChecking=accept-new")
                .arg("-o")
                .arg("IdentitiesOnly=yes")
                .arg("-o")
                .arg("PreferredAuthentications=password");

            if let Some(identity) = ssh_identity_file {
                sudo_setup.arg("-i").arg(identity);
            }

            sudo_setup.arg(format!("{}@{}", target_user, target_ip))
                .arg(&sudo_cmd);

            let sudo_output = sudo_setup.output().map_err(|e| format!("Failed to configure sudo: {}", e))?;
            if !sudo_output.status.success() {
                let _ = app.emit("deploy-progress", format!("Warning: Failed to auto-configure passwordless sudo. It might already be set. Stderr: {}", String::from_utf8_lossy(&sudo_output.stderr)));
            } else {
                let _ = app.emit("deploy-progress", "Passwordless sudo successfully configured.");
            }
        }
    }
    Ok(())
}

#[tauri::command]
async fn deploy(app: tauri::AppHandle, config: DeployConfig) -> Result<DeployResult, String> {
    let app_dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
    let deploy_dir = app_dir.join("deploy");
    
    let _ = app.emit("deploy-progress", "Generating NixOS configuration files...");
    generate_nix_files(&deploy_dir, &config)?;
    let _ = app.emit("deploy-progress", format!("Configuration files written to {:?}", deploy_dir));

    let _ = app.emit("deploy-progress", "Configuring target server SSH access...");
    // Copy SSH key if password is provided
    run_ssh_copy_id(
        &config.target_user,
        &config.target_ip,
        config.ssh_password.as_deref(),
        config.ssh_identity_file.as_deref(),
        &app
    ).await?;

    let _ = app.emit("deploy-progress", "Starting NixOS-Anywhere deployment. This may take several minutes...");

    // Run nixos-anywhere
     let mut cmd = system_command("nix");
     let flake_path = deploy_dir.canonicalize().map_err(|e| e.to_string())?;
     let flake_arg = format!("{}#{}", flake_path.display(), config.hostname);
     
     let path_env = std::env::var("PATH").unwrap_or_else(|_| "".to_string());
     let combined_path = format!("{}:/run/current-system/sw/bin:/nix/var/nix/profiles/default/bin:/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin", path_env);
     cmd.env("PATH", &combined_path);

     cmd.arg("--extra-experimental-features")
         .arg("nix-command flakes")
         .arg("run")
         .arg("github:nix-community/nixos-anywhere")
         .arg("--")
         .arg("-L")
         .arg("--generate-hardware-config")
         .arg("nixos-generate-config")
         .arg("./hardware-configuration.nix")
         .arg("--flake")
         .arg(flake_arg)
         .arg("--extra-files")
         .arg(deploy_dir.join("extra-files"))
         .arg("--ssh-option")
         .arg("IdentitiesOnly=yes")
         .arg("--ssh-option")
         .arg("StrictHostKeyChecking=accept-new");

    let default_key = app.path().app_data_dir().unwrap_or_default().join("ssh/id_ed25519");
    let identity = if let Some(ref id) = config.ssh_identity_file {
        id.clone()
    } else if default_key.exists() {
        default_key.to_string_lossy().to_string()
    } else {
        "".to_string()
    };

    if !identity.is_empty() {
        cmd.arg("-i").arg(&identity);
    }
    
    if let Some(ref password) = config.ssh_password {
        cmd.env("SSHPASS", password);
        cmd.arg("--env-password");
    }

    cmd.arg("--target-host")
       .arg(format!("{}@{}", config.target_user, config.target_ip));
    println!("{:?}", deploy_dir);

    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());

    let mut child = cmd
        .current_dir(&deploy_dir)
        .spawn()
        .map_err(|e| format!("Failed to execute nixos-anywhere: {}", e))?;

    let stdout = child.stdout.take().unwrap();
    let stderr = child.stderr.take().unwrap();

    let app_clone1 = app.clone();
    let app_clone2 = app.clone();

    let thread_out = thread::spawn(move || {
        let reader = BufReader::new(stdout);
        for line in reader.lines() {
            if let Ok(line) = line {
                let _ = app_clone1.emit("deploy-progress", line);
            }
        }
    });

    let thread_err = thread::spawn(move || {
        let reader = BufReader::new(stderr);
        for line in reader.lines() {
            if let Ok(line) = line {
                let _ = app_clone2.emit("deploy-progress", line);
            }
        }
    });

    let status = child.wait().map_err(|e| format!("Failed to wait for process: {}", e))?;
    let _ = thread_out.join();
    let _ = thread_err.join();

    if status.success() {
        Ok(DeployResult {
            success: true,
            message: "Deployment successful!".to_string(),
        })
    } else {
        Ok(DeployResult {
            success: false,
            message: format!("Deployment failed with status: {}", status),
        })
    }
}

#[tauri::command]
async fn deploy_existing(app: tauri::AppHandle, config: ExistingDeployConfig) -> Result<DeployResult, String> {
    let deploy_dir = PathBuf::from(&config.flake_dir);
    
    let _ = app.emit("deploy-progress", format!("Preparing deployment using existing configuration at {:?}", deploy_dir));
    let _ = app.emit("deploy-progress", "Configuring target server SSH access...");

    // Copy SSH key if password is provided
    run_ssh_copy_id(
        &config.target_user,
        &config.target_ip,
        config.ssh_password.as_deref(),
        config.ssh_identity_file.as_deref(),
        &app
    ).await?;

    let _ = app.emit("deploy-progress", "Detecting NixOS configuration from flake.nix...");

    // Automatically detect the configuration name from flake.nix
    let show_output = system_command("nix")
        .args(["--extra-experimental-features", "nix-command flakes", "flake", "show", "--json"])
        .current_dir(&deploy_dir)
        .output()
        .map_err(|e| format!("Failed to run nix flake show: {}", e))?;

    if !show_output.status.success() {
        return Err(format!("nix flake show failed: {}", String::from_utf8_lossy(&show_output.stderr)));
    }

    let v: serde_json::Value = serde_json::from_slice(&show_output.stdout)
        .map_err(|e| format!("Failed to parse nix flake show output: {}", e))?;

    let config_name = v["nixosConfigurations"]
        .as_object()
        .and_then(|obj| obj.keys().next())
        .ok_or_else(|| "No NixOS configurations found in flake.nix".to_string())?;

    let _ = app.emit("deploy-progress", format!("Detected configuration: {}", config_name));
    let _ = app.emit("deploy-progress", "Starting NixOS-Anywhere deployment. This may take several minutes...");

     let mut cmd = system_command("nix");
     let flake_arg = format!(".#{}", config_name);
     
     let path_env = std::env::var("PATH").unwrap_or_else(|_| "".to_string());
     let combined_path = format!("{}:/run/current-system/sw/bin:/nix/var/nix/profiles/default/bin:/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin", path_env);
     cmd.env("PATH", &combined_path);

     cmd.arg("--extra-experimental-features")
         .arg("nix-command flakes")
         .arg("run")
         .arg("github:nix-community/nixos-anywhere")
         .arg("--")
         .arg("-L")
         .arg("--generate-hardware-config")
         .arg("nixos-generate-config")
         .arg("./hardware-configuration.nix")
         .arg("--flake")
         .arg(flake_arg)
         .arg("--ssh-option")
         .arg("IdentitiesOnly=yes")
         .arg("--ssh-option")
         .arg("StrictHostKeyChecking=accept-new");

    // Automatically detect and pass extra-files if the folder exists in the flake directory
    let extra_files_path = deploy_dir.join("extra-files");
    if extra_files_path.exists() {
        cmd.arg("--extra-files").arg(extra_files_path);
    }

    let default_key = app.path().app_data_dir().unwrap_or_default().join("ssh/id_ed25519");
    let identity = if let Some(ref id) = config.ssh_identity_file {
        id.clone()
    } else if default_key.exists() {
        default_key.to_string_lossy().to_string()
    } else {
        "".to_string()
    };

    if !identity.is_empty() {
        cmd.arg("-i").arg(&identity);
    }

    if let Some(ref password) = config.ssh_password {
        cmd.env("SSHPASS", password);
        cmd.arg("--env-password");
    }

    cmd.arg("--target-host")
       .arg(format!("{}@{}", config.target_user, config.target_ip));

    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());

    let mut child = cmd
        .current_dir(&deploy_dir)
        .spawn()
        .map_err(|e| format!("Failed to execute nixos-anywhere: {}", e))?;

    let stdout = child.stdout.take().unwrap();
    let stderr = child.stderr.take().unwrap();

    let app_clone1 = app.clone();
    let app_clone2 = app.clone();

    let thread_out = thread::spawn(move || {
        let reader = BufReader::new(stdout);
        for line in reader.lines() {
            if let Ok(line) = line {
                let _ = app_clone1.emit("deploy-progress", line);
            }
        }
    });

    let thread_err = thread::spawn(move || {
        let reader = BufReader::new(stderr);
        for line in reader.lines() {
            if let Ok(line) = line {
                let _ = app_clone2.emit("deploy-progress", line);
            }
        }
    });

    let status = child.wait().map_err(|e| format!("Failed to wait for process: {}", e))?;
    let _ = thread_out.join();
    let _ = thread_err.join();

    if status.success() {
        Ok(DeployResult {
            success: true,
            message: "Deployment successful!".to_string(),
        })
    } else {
        Ok(DeployResult {
            success: false,
            message: format!("Deployment failed with status: {}", status),
        })
    }
}

#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Rust!", name)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    setup_env();
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

fn setup_env() {
    let path = std::env::var("PATH").unwrap_or_else(|_| "".to_string());
    let mut paths: Vec<std::path::PathBuf> = std::env::split_paths(&path).collect();

    let extra_paths = [
        "/run/current-system/sw/bin",
        "/nix/var/nix/profiles/default/bin",
        "/usr/local/bin",
        "/usr/local/sbin",
        "/usr/bin",
        "/bin",
        "/usr/sbin",
        "/sbin",
    ];

    let mut changed = false;
    for p in extra_paths {
        let path_buf = std::path::PathBuf::from(p);
        if !paths.contains(&path_buf) {
            paths.insert(0, path_buf);
            changed = true;
        }
    }

    if let Ok(home) = std::env::var("HOME") {
        let nix_profile = std::path::PathBuf::from(home).join(".nix-profile/bin");
        if !paths.contains(&nix_profile) {
            paths.insert(0, nix_profile);
            changed = true;
        }
    }

    if changed {
        if let Ok(new_path) = std::env::join_paths(paths) {
            std::env::set_var("PATH", new_path);
        }
    }
}
