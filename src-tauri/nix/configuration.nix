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

  system.stateVersion = "24.05";
}
