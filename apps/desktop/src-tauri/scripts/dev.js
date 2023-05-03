const { spawn } = require('./spawn.js');
const { setupPlatformEnv } = require('./env.js');

setupPlatformEnv();

spawn('pnpm', ['tauri', 'dev']).catch((code) => process.exit(code));
