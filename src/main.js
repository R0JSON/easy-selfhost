const { invoke } = window.__TAURI__.core;

window.addEventListener("DOMContentLoaded", () => {
  const form = document.querySelector("#deploy-form");
  const deployBtn = document.querySelector("#deploy-btn");
  const statusContainer = document.querySelector("#status-container");
  const statusBorder = document.querySelector("#status-border");
  const statusTitle = document.querySelector("#status-title");
  const statusMsg = document.querySelector("#status-msg");
  const sslEnable = document.querySelector("#ssl_enable");
  const acmeEmailContainer = document.querySelector("#acme-email-container");

  sslEnable.addEventListener("change", () => {
    if (sslEnable.checked) {
      acmeEmailContainer.classList.remove("hidden");
      document.querySelector("#acme_email").required = true;
    } else {
      acmeEmailContainer.classList.add("hidden");
      document.querySelector("#acme_email").required = false;
    }
  });

  form.addEventListener("submit", async (e) => {
    e.preventDefault();

    const config = {
      target_ip: document.querySelector("#target_ip").value,
      target_user: document.querySelector("#target_user").value,
      hostname: document.querySelector("#hostname").value,
      ssh_key: document.querySelector("#ssh_key").value,
      target_device: document.querySelector("#target_device").value,
      nextcloud_hostname: document.querySelector("#nextcloud_hostname").value,
      admin_password: document.querySelector("#admin_password").value,
      ssl_enable: document.querySelector("#ssl_enable").checked,
      acme_email: document.querySelector("#acme_email").value,
    };

    // Show status
    statusContainer.classList.remove("hidden");
    statusBorder.className = "bg-white shadow-md rounded-lg p-6 border-l-4 border-blue-500";
    statusTitle.textContent = "Deploying...";
    statusMsg.textContent = "Generating Nix configurations and starting nixos-anywhere...";
    deployBtn.disabled = true;
    deployBtn.textContent = "Deploying...";

    try {
      const result = await invoke("deploy", { config });
      
      if (result.success) {
        statusBorder.className = "bg-white shadow-md rounded-lg p-6 border-l-4 border-green-500";
        statusTitle.textContent = "Success!";
        statusMsg.textContent = result.message;
      } else {
        statusBorder.className = "bg-white shadow-md rounded-lg p-6 border-l-4 border-red-500";
        statusTitle.textContent = "Error";
        statusMsg.textContent = result.message;
      }
    } catch (err) {
      statusBorder.className = "bg-white shadow-md rounded-lg p-6 border-l-4 border-red-500";
      statusTitle.textContent = "Critical Error";
      statusMsg.textContent = err;
    } finally {
      deployBtn.disabled = false;
      deployBtn.textContent = "Deploy Now";
    }
  });
});
