FROM node:lts-slim

WORKDIR /app

# Update default packages
RUN apt-get update

# Get Ubuntu packages
RUN apt-get install -y \
  build-essential \
  curl \
  vim

# Install pnpm
RUN curl -fsSL https://get.pnpm.io/install.sh | PNPM_VERSION=7.0.0-rc.9 sh -
# Install Rust
RUN curl https://sh.rustup.rs -sSf | bash -s -- -y

# Set the PATH to include the pnpm and rust executables
ENV PATH="/root/.cargo/bin:/root/.local/share/pnpm:${PATH}"
ENV PNPM_HOME="/root/.local/share/pnpm"

COPY .github/scripts/setup-system.sh .github/scripts/setup-system.sh

# Setup the system
RUN ["/bin/bash", ".github/scripts/setup-system.sh"]

COPY . .

RUN pnpm i -w react react-dom
RUN pnpm i
RUN pnpm prep
