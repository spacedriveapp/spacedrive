// Dev stub for private `@spacebot/api-client` (see apps/tauri/stubs for details).

const asyncNoop = async (): Promise<undefined> => undefined;

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
