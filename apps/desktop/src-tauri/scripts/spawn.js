const { spawn } = require('node:child_process');

module.exports.spawn = (command, args) => {
	if (typeof command !== 'string' || command.length === 0)
		throw new Error('Command must be a string and not empty');

	if (args == null) args = [];
	else if (!Array.isArray(args) || args.some((arg) => typeof arg !== 'string'))
		throw new Error('Args must be an array of strings');

	return new Promise((resolve, reject) => {
		const child = spawn(command, args, { shell: true, stdio: 'inherit' });
		process.on('SIGTERM', () => child.kill('SIGTERM'));
		process.on('SIGINT', () => child.kill('SIGINT'));
		process.on('SIGBREAK', () => child.kill('SIGBREAK'));
		process.on('SIGHUP', () => child.kill('SIGHUP'));
		child.on('error', (error) => {
			console.error(error);
			reject(1);
		});
		child.on('exit', (code, signal) => {
			if (code === null) code = signal === 'SIGINT' ? 0 : 1;
			if (code === 0) {
				resolve();
			} else {
				reject(code);
			}
		});
	});
};
