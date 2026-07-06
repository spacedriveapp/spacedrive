// Local dev stub for the private `@spacebot/api-client` package.
//
// `@spacebot/api-client` lives in Spacedrive's separate (non-public) `spacebot`
// companion repo and is only aliased when that repo is checked out as a sibling
// (see `hasSpacebot` in vite.config.ts). The production build already marks the
// specifier as `external`, but the dev server had no equivalent fallback, so
// Vite failed to resolve the Spacebot imports that `router.tsx` pulls in eagerly
// — which blanked the entire app.
//
// This stub lets the dev server resolve those imports so the core file-explorer
// UI loads. The Spacebot (AI assistant) feature itself is inert without the real
// client: any call is a no-op. Remove this + the alias once the real spacebot
// repo is available.

const asyncNoop = async (): Promise<undefined> => undefined;

// Any property access returns an async no-op callable, so Spacebot code that
// touches `apiClient.*` degrades gracefully instead of throwing.
export const apiClient: any = new Proxy(
	{},
	{
		get() {
			return (..._args: any[]) => asyncNoop();
		},
	}
);

export const getEventsUrl = (): string => '';
export const setServerUrl = (_url?: string): void => {};

export default apiClient;
