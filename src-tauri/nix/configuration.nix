{ config, pkgs, ... }: {
  networking.hostName = "{{ hostname }}";
  
  boot.loader.grub.enable = true;
  boot.loader.grub.efiSupport = true;
  boot.loader.grub.efiInstallAsRemovable = true; # Works better for VMs and some hardware
  boot.loader.grub.device = "nodev";
  boot.initrd.availableKernelModules = [ "virtio_pci" "virtio_blk" "virtio_scsi" "virtio_net" "virtio_console" "xhci_pci" "ahci" "usbhid" "sr_mod" "ata_piix" "uhci_hcd" "sd_mod" ];

  services.openssh.enable = true;
  users.users.root.openssh.authorizedKeys.keys = [
    "{{ ssh_key }}"
  ];

  networking.firewall.allowedTCPPorts = [ 80 443 ];

  services.nginx.enable = true;

  {{ nextcloud_block }}
  {{ nextcloud_nginx_vhost }}

  {{ acme_config }}

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

  system.stateVersion = "24.05";
}
