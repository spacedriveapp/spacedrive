import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { inspect } from 'node:util';
import { defineConfig } from 'cypress';

const __filename = fileURLToPath(import.meta.url);
const __dirname = dirname(__filename);

const ci_specific = {
	// Double all the default timeouts
	// https://docs.cypress.io/guides/references/configuration#Timeouts
	defaultCommandTimeout: 4000 * 2,
	execTimeout: 60000 * 2,
	taskTimeout: 60000 * 2,
	pageLoadTimeout: 60000 * 2,
	requestTimeout: 5000 * 2,
	responseTimeout: 30000 * 2,
	// Enable test retries
	// https://docs.cypress.io/guides/guides/test-retries#Configure-retry-attempts-for-all-modes
	retries: 2
};

const config = defineConfig({
	e2e: {
		baseUrl: 'http://localhost:8002',
		setupNodeEvents(on) {
			on('task', {
				repoRoot: () => {
					return resolve(join(__dirname, '../../'));
				}
			});
		}
	},
	video: true,
	experimentalWebKitSupport: true,
	...(process.env.CI === 'true' ? ci_specific : {})
});

console.log('Cypress default config:', inspect(config, { depth: null, colors: true }));

export default config;
