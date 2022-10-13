import { createClient } from '@rspc/client';
import { createSolidQueryHooks } from '@rspc/solid';
import { TauriTransport } from '@rspc/tauri';
import { QueryClient } from '@tanstack/solid-query';

import type { Procedures } from './bindings';

// These were the bindings exported from your Rust code!

// You must provide the generated types as a generic and create a transport (in this example we are using HTTP Fetch) so that the client knows how to communicate with your API.
export const rspcClient = createClient<Procedures>({
	// Refer to the integration your using for the correct transport.
	transport: new TauriTransport()
});

export const queryClient = new QueryClient({
	defaultOptions: {
		queries: {
			suspense: true
		}
	}
});

export const rspc = createSolidQueryHooks<Procedures>();
