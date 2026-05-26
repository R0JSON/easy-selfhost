{ config, pkgs, ... }: {
  networking.hostName = "{{ hostname }}";
  
  boot.loader.grub.enable = true;
  boot.loader.grub.efiSupport = true;
  boot.loader.grub.efiInstallAsRemovable = true;
  boot.loader.grub.device = "nodev";
  boot.initrd.availableKernelModules = [ "virtio_pci" "virtio_blk" "virtio_scsi" "virtio_net" "virtio_console" "xhci_pci" "ahci" "usbhid" "sr_mod" "ata_piix" "uhci_hcd" "sd_mod" ];

  services.openssh.enable = true;
  users.users.root.openssh.authorizedKeys.keys = [
    "{{ ssh_key }}"
  ];

  networking.firewall.allowedTCPPorts = [ 80 443 8443 53 ];
  networking.firewall.allowedUDPPorts = [ 53 51820 ];

  services.nginx.enable = true;

  {{ acme_config }}

  {{ nextcloud_block }}
  {{ nextcloud_nginx_vhost }}

  {{ jellyfin_block }}
  {{ jellyfin_nginx_vhost }}

  services.vaultwarden = {
    enable = {{ vaultwarden_enable }};
    config = {
      DOMAIN = "https://{{ vaultwarden_hostname }}";
      SIGNUPS_ALLOWED = {{ vaultwarden_signups }};
      ROCKET_PORT = 8222;
      ROCKET_ADDRESS = "127.0.0.1";
    };
  };
  {{ vaultwarden_nginx_vhost }}

  {{ immich_block }}
  {{ immich_nginx_vhost }}

  {{ gitea_block }}
  {{ gitea_nginx_vhost }}

  {{ uptime_kuma_block }}
  {{ uptime_kuma_nginx_vhost }}

  {{ vikunja_block }}
  {{ vikunja_nginx_vhost }}

  {{ tailscale_block }}

  {{ adguardhome_block }}
  {{ adguardhome_nginx_vhost }}

  system.stateVersion = "24.05";
}