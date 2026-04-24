const { invoke } = window.__TAURI__.core;

window.addEventListener("DOMContentLoaded", async () => {
  // Screens
  const screenHome = document.querySelector("#screen-home");
  const screenConfig = document.querySelector("#screen-config");
  const screenDeployTarget = document.querySelector("#screen-deploy-target");
  const screenDeployExisting = document.querySelector("#screen-deploy-existing");
  const screenDeployProgress = document.querySelector("#screen-deploy-progress");

  // Navigation Buttons
  const btnGotoCreate = document.querySelector("#btn-goto-create");
  const btnGotoDeploy = document.querySelector("#btn-goto-deploy");
  const btnBacks = document.querySelectorAll(".btn-back");
  const btnBackConfig = document.querySelector("#btn-back-config");
  const btnBackHomeProgress = document.querySelector("#btn-back-home-progress");

  // Forms and actions
  const configForm = document.querySelector("#config-form");
  const deployTargetForm = document.querySelector("#deploy-target-form");
  const saveConfigBtn = document.querySelector("#save-config-btn");
  const deployExistingForm = document.querySelector("#deploy-existing-form");
  const pickDirBtn = document.querySelector("#pick-dir-btn");

  const sslEnable = document.querySelector("#ssl_enable");
  const acmeEmailContainer = document.querySelector("#acme-email-container");

  // Dependency UI
  const depsContainer = document.querySelector("#deps-container");
  const depsMsg = document.querySelector("#deps-msg");

  // Progress UI
  const progressTitle = document.querySelector("#progress-title");
  const progressIndicator = document.querySelector("#progress-indicator");
  const progressStatusText = document.querySelector("#progress-status-text");
  const progressLogs = document.querySelector("#progress-logs");
  const spinner = document.querySelector(".spinner");

  // Navigation Logic
  function showScreen(screen) {
    screenHome.classList.add("hidden");
    screenConfig.classList.add("hidden");
    screenDeployTarget.classList.add("hidden");
    screenDeployExisting.classList.add("hidden");
    screenDeployProgress.classList.add("hidden");
    screen.classList.remove("hidden");
  }

  btnGotoCreate.addEventListener("click", () => showScreen(screenConfig));
  btnGotoDeploy.addEventListener("click", () => showScreen(screenDeployExisting));
  btnBacks.forEach(btn => btn.addEventListener("click", () => showScreen(screenHome)));
  btnBackConfig.addEventListener("click", () => showScreen(screenConfig));
  btnBackHomeProgress.addEventListener("click", () => showScreen(screenHome));

  // SSL Checkbox Logic
  sslEnable.addEventListener("change", () => {
    if (sslEnable.checked) {
      acmeEmailContainer.classList.remove("hidden");
      document.querySelector("#acme_email").required = true;
    } else {
      acmeEmailContainer.classList.add("hidden");
      document.querySelector("#acme_email").required = false;
    }
  });

  // Service toggle logic: show/hide config panels when checkboxes are toggled
  function setupServiceToggle(checkboxId, configPanelId, requiredFieldIds = []) {
    const checkbox = document.querySelector(`#${checkboxId}`);
    const panel = document.querySelector(`#${configPanelId}`);
    if (!checkbox || !panel) return;

    checkbox.addEventListener("change", () => {
      if (checkbox.checked) {
        panel.classList.add("active");
        requiredFieldIds.forEach(id => {
          const el = document.querySelector(`#${id}`);
          if (el) el.required = true;
        });
      } else {
        panel.classList.remove("active");
        requiredFieldIds.forEach(id => {
          const el = document.querySelector(`#${id}`);
          if (el) el.required = false;
        });
      }
    });
  }

  setupServiceToggle("svc_jellyfin", "jellyfin-config", ["jellyfin_hostname", "jellyfin_media_dir"]);
  setupServiceToggle("svc_vaultwarden", "vaultwarden-config", ["vaultwarden_hostname"]);

  // SSH Key Generation
  const generateKeyBtn = document.querySelector("#generate-key-btn");
  const sshKeyArea = document.querySelector("#ssh_key");
  const sshIdentityInput = document.querySelector("#ssh_identity_file");
  const keyGenMsg = document.querySelector("#key-gen-msg");

  generateKeyBtn.addEventListener("click", async () => {
    try {
      generateKeyBtn.disabled = true;
      generateKeyBtn.textContent = "Generating...";
      const result = await invoke("generate_ssh_key");
      sshKeyArea.value = result.public_key;
      sshIdentityInput.value = result.private_key_path;
      keyGenMsg.textContent = `New key generated! Private key saved at: ${result.private_key_path}`;
      keyGenMsg.classList.remove("hidden");
    } catch (err) {
      alert("Failed to generate key: " + err);
    } finally {
      generateKeyBtn.disabled = false;
      generateKeyBtn.textContent = "Generate New Key";
    }
  });

  // Vaultwarden admin token generator
  const generateVwTokenBtn = document.querySelector("#generate-vw-token-btn");
  const vwTokenInput = document.querySelector("#vaultwarden_admin_token");

  generateVwTokenBtn.addEventListener("click", () => {
    // Generate a random 48-char hex token without calling backend
    const array = new Uint8Array(24);
    crypto.getRandomValues(array);
    vwTokenInput.value = Array.from(array).map(b => b.toString(16).padStart(2, "0")).join("");
    vwTokenInput.type = "text";
    setTimeout(() => { vwTokenInput.type = "password"; }, 3000);
  });

  // Check Dependencies on Startup
  try {
    const deps = await invoke("check_dependencies");
    const missing = [];
    if (!deps.nix) missing.push("nix");
    if (!deps.ssh) missing.push("ssh");

    if (missing.length > 0) {
      depsMsg.textContent = `Missing required tools: ${missing.join(", ")}. Please install them.`;
      depsContainer.classList.remove("hidden");
    }
  } catch (e) {
    console.error("Failed to check dependencies", e);
  }

  // Next step in wizard
  configForm.addEventListener("submit", (e) => {
    e.preventDefault();
    showScreen(screenDeployTarget);
  });

  // Get Config from Form
  function getCreateConfig() {
    const jellyfinEnabled = document.querySelector("#svc_jellyfin").checked;
    const vaultwardenEnabled = document.querySelector("#svc_vaultwarden").checked;

    return {
      // Connection (filled in step 2)
      target_ip: document.querySelector("#target_ip")?.value || "",
      target_user: document.querySelector("#target_user")?.value || "",

      // System
      hostname: document.querySelector("#hostname").value,
      ssh_key: document.querySelector("#ssh_key").value,
      target_device: document.querySelector("#target_device").value,

      // Nextcloud (always enabled)
      nextcloud_hostname: document.querySelector("#nextcloud_hostname").value,
      admin_password: document.querySelector("#admin_password").value,
      ssl_enable: document.querySelector("#ssl_enable").checked,
      acme_email: document.querySelector("#acme_email").value,

      // SSH identity
      ssh_identity_file: sshIdentityInput.value || null,

      // Jellyfin
      jellyfin_enable: jellyfinEnabled,
      jellyfin_hostname: jellyfinEnabled ? document.querySelector("#jellyfin_hostname").value : null,
      jellyfin_media_dir: jellyfinEnabled ? document.querySelector("#jellyfin_media_dir").value : null,
      //jellyfin_hw_accel: jellyfinEnabled ? document.querySelector("#jellyfin_hw_accel").value : null,
      jellyfin_open_firewall: jellyfinEnabled ? document.querySelector("#jellyfin_open_firewall").checked : false,

      // Vaultwarden
      vaultwarden_enable: vaultwardenEnabled,
      vaultwarden_hostname: vaultwardenEnabled ? document.querySelector("#vaultwarden_hostname").value : null,
      vaultwarden_port: vaultwardenEnabled ? parseInt(document.querySelector("#vaultwarden_port").value, 10) : null,
      vaultwarden_admin_token: vaultwardenEnabled ? (document.querySelector("#vaultwarden_admin_token").value || null) : null,
      vaultwarden_signups_allowed: vaultwardenEnabled ? document.querySelector("#vaultwarden_signups").checked : false,
    };
  }

  function initProgressScreen(title, statusText) {
    progressTitle.textContent = title;
    progressStatusText.textContent = statusText;
    progressStatusText.className = "text-lg text-gray-600 font-medium";
    progressLogs.textContent = "Waiting for output...\n";
    progressLogs.className = "bg-gray-900 text-gray-100 font-mono text-sm p-4 rounded-md flex-grow overflow-y-auto max-h-96 whitespace-pre-wrap";
    spinner.style.display = "block";
    btnBackHomeProgress.classList.add("hidden");
    showScreen(screenDeployProgress);
  }

  function completeProgressScreen(isSuccess, message) {
    spinner.style.display = "none";
    btnBackHomeProgress.classList.remove("hidden");
    if (isSuccess) {
      progressTitle.textContent = "Deployment Successful";
      progressStatusText.textContent = "Completed without errors.";
      progressStatusText.className = "text-lg text-green-600 font-medium";
      progressLogs.textContent += "\n" + message;
    } else {
      progressTitle.textContent = "Deployment Failed";
      progressStatusText.textContent = "An error occurred.";
      progressStatusText.className = "text-lg text-red-600 font-medium";
      progressLogs.textContent += "\n" + message;
      progressLogs.classList.add("border", "border-red-500");
    }
  }

  // Deploy Generated Config
  deployTargetForm.addEventListener("submit", async (e) => {
    e.preventDefault();
    const config = getCreateConfig();

    const services = ["Nextcloud"];
    if (config.jellyfin_enable) services.push("Jellyfin");
    if (config.vaultwarden_enable) services.push("Vaultwarden");

    initProgressScreen(
      `Deploying Configuration (${services.join(", ")})...`,
      "Generating Nix files and starting nixos-anywhere..."
    );

    try {
      const result = await invoke("deploy", { config });
      completeProgressScreen(result.success, result.message);
    } catch (err) {
      completeProgressScreen(false, String(err));
    }
  });

  // Save Config
  saveConfigBtn.addEventListener("click", async () => {
    if (!configForm.checkValidity()) {
      configForm.reportValidity();
      return;
    }
    const config = getCreateConfig();
    try {
      const savePath = await invoke('plugin:dialog|open', {
        options: {
          directory: true,
          multiple: false,
          title: "Select Directory to Save Configuration"
        }
      });
      if (savePath) {
        const result = await invoke("save_configuration", { config, savePath });
        if (result.success) {
          alert(`Configuration Saved: ${result.message}`);
        } else {
          alert(`Save Failed: ${result.message}`);
        }
      }
    } catch (err) {
      alert(`Save Error: ${err}`);
    }
  });

  // Pick Directory for Existing Deployment
  pickDirBtn.addEventListener("click", async () => {
    try {
      const selected = await invoke('plugin:dialog|open', {
        options: {
          directory: true,
          multiple: false,
          title: "Select Nix Flake Directory"
        }
      });
      if (selected) {
        document.querySelector("#flake_dir").value = selected;
      }
    } catch (err) {
      console.error("Failed to open dialog", err);
    }
  });

  // Deploy Existing Config
  deployExistingForm.addEventListener("submit", async (e) => {
    e.preventDefault();
    
    const config = {
      flake_dir: document.querySelector("#flake_dir").value,
      target_ip: document.querySelector("#existing_target_ip").value,
      target_user: document.querySelector("#existing_target_user").value,
      ssh_identity_file: document.querySelector("#existing_ssh_identity").value || null,
      admin_password: document.querySelector("#existing_admin_pwd").value || null,
    };

    initProgressScreen("Deploying Existing Configuration...", "Starting nixos-anywhere with existing flake...");

    try {
      const result = await invoke("deploy_existing", { config });
      completeProgressScreen(result.success, result.message);
    } catch (err) {
      completeProgressScreen(false, String(err));
    }
  });
});
