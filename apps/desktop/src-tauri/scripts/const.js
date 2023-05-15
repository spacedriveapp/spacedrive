const path = require('node:path');

const platform = /^(msys|cygwin)$/.test(process.env.OSTYPE ?? '') ? 'win32' : process.platform;

module.exports = {
	platform,
	workspace: path.resolve(__dirname, '../../../../'),
	setupScript: `.github/scripts/${platform === 'win32' ? 'setup-system.ps1' : 'setup-system.sh'}`
};
