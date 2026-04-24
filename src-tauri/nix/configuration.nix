{ config, pkgs, ... }: {
  networking.hostName = "{{ hostname }}";
  services.openssh.enable = true;
  users.users.root.openssh.authorizedKeys.keys = [
    "{{ ssh_key }}"
  ];

  services.nextcloud = {
    enable = true;
    hostName = "{{ nextcloud_hostname }}";
    package = pkgs.nextcloud29;
    
    database.createLocally = true;
    configureRedis = true;
    
    config = {
      dbtype = "pgsql";
      adminuser = "admin";
      adminpassFile = "/etc/nextcloud-admin-pass";
    };
  };

  services.nginx.virtualHosts."{{ nextcloud_hostname }}" = {
    forceSSL = {{ ssl_enable }};
    enableACME = {{ ssl_enable }};
  };

  {{ acme_config }}

  {{ jellyfin_block }}

  services.vaultwarden = {
    enable = {{ vaultwarden_enable }};
    config = {
      DOMAIN = "https://{{ vaultwarden_hostname }}";
      SIGNUPS_ALLOWED = {{ vaultwarden_signups }};
      ROCKET_PORT = {{ vaultwarden_port }};
      ROCKET_ADDRESS = "127.0.0.1";
    };
  };

  services.caddy = {
    enable = true;
    globalConfig = "auto_https disable_redirects";
    virtualHosts."{{ vaultwarden_hostname }}" = {
      extraConfig = "
        tls internal
        reverse_proxy 127.0.0.1:{{ vaultwarden_port }}
      ";
    };
  };

  system.stateVersion = "24.05";
}
