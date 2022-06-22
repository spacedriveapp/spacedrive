sudo npm install -y -g npm@latest pnpm
sudo apt install -y cargo pnpm

# OS specific part
brew install ffmpeg

pnpm i
cargo install tauri-cli
pnpm prep
