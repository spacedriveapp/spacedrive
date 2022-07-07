# OS specific part
brew install ffmpeg pnpm
brew upgrade
brew cleanup
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs > rustup.sh
chmod +x rustup.sh
sudo ./rustup.sh -y
rm -f rustup.sh
while read -r env; do export "$env"; done
pnpm i
cargo install tauri-cli
pnpm prep
