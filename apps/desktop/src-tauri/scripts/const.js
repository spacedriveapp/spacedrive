const path = require('node:path');

module.exports = {
	platform: /^(msys|cygwin)$/.test(process.env.OSTYPE ?? '') ? 'win32' : process.platform,
	workspace: path.resolve(__dirname, '../../../../'),
	setupScript: `.github/scripts/${isWindows ? 'setup-system.ps1' : 'setup-system.sh'}`
}
