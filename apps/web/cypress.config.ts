import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
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
	responseTimeout: 30000 * 2
};

export default defineConfig({
	e2e: {
		baseUrl: 'http://localhost:8002',
		setupNodeEvents(on, config) {
			on('task', {
				repoRoot: () => {
					return resolve(join(__dirname, '../../'));
				}
			});
		}
	},
	video: true,
	...(process.env.CI === 'true' ? ci_specific : {})
});
