# Tauri + Vanilla

This template should help get you started developing with Tauri in vanilla HTML, CSS and Javascript.

Install dependencies:
```bash
sudo apt update && sudo apt install -y \
  build-essential \
  curl \
  wget \
  file \
  libxdo-dev \
  libssl-dev \
  libayatana-appindicator3-dev \
  librsvg2-dev \
  libgtk-3-dev \
  libwebkit2gtk-4.1-dev \
  libsoup-3.0-dev \
  libjavascriptcoregtk-4.1-dev \
  nodejs \
  npm 

```
Clone the repo
```bash
git clone https://github.com/R0JSON/easy-selfhost
cd easy-selfhost
```

Install rust
```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```
Install tauri
```bash
sudo npm install -g @tauri-apps/cli
```
Run the app
```bash
npm run tauri dev
```
## Recommended IDE Setup

- [VS Code](https://code.visualstudio.com/) + [Tauri](https://marketplace.visualstudio.com/items?itemName=tauri-apps.tauri-vscode) + [rust-analyzer](https://marketplace.visualstudio.com/items?itemName=rust-lang.rust-analyzer)
