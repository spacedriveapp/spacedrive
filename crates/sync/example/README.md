# Create rspc app

This app was scaffolded using the [create-rspc-app](https://rspc.dev) CLI.

## Usage

```bash
# Terminal One
cd web
pnpm i
pnpm dev

# Terminal Two
cd api/
cargo prisma generate
cargo prisma db push
cargo run
```
