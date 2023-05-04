const { spawn } = require('./spawn.js');
const { setupPlatformEnv } = require('./env.js');

setupPlatformEnv(null, true);

spawn('pnpm', ['tauri', 'dev']).catch((code) => process.exit(code));
