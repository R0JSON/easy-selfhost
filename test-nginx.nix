{ pkgs, ... }: {
  services.nginx = {
    enable = true;
    virtualHosts."test" = {
      forceSSL = false;
      enableACME = false;
      locations."/".proxyPass = "http://127.0.0.1:8000";
    };
  };
}
