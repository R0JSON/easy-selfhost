{ pkgs, ... }: {
  services.vaultwarden = {
    enable = true;
    
    config = {
      DOMAIN = "https://vault.example.com";
      SIGNUPS_ALLOWED = true;
      ROCKET_PORT = 8222;
      ROCKET_ADDRESS = "127.0.0.1";
    };
  };
}
