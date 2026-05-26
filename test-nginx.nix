{ pkgs, ... }: {
  services.nginx.enable = true;
  services.nginx.virtualHosts."test" = {
    forceSSL = true;
  };
}
