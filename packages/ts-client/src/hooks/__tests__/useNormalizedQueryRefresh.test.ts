import { describe, expect, it } from "bun:test";
import { QueryClient, QueryObserver } from "@tanstack/react-query";
import { handleResourceEvent, REFRESH_SETTLE_MS } from "../useNormalizedQuery";

const options = {
	query: "files.directory_listing",
	input: {},
	resourceType: "file",
};
const queryKey = ["query:files.directory_listing", "library", { path: "/a" }];
const refresh = () => JSON.parse('"Refresh"');
const sleep = (ms: number) => new Promise((resolve) => setTimeout(resolve, ms));

function directoryQuery(fetchDelayMs: number) {
	const queryClient = new QueryClient({
		defaultOptions: { queries: { retry: false, gcTime: Infinity } },
	});
	let fetches = 0;
	const observer = new QueryObserver(queryClient, {
		queryKey,
		queryFn: async () => {
			const fetch = ++fetches;
			await sleep(fetchDelayMs);
			return { files: [], fetch };
		},
	});
	const unsubscribe = observer.subscribe(() => {});
	return { queryClient, observer, unsubscribe, fetches: () => fetches };
}

describe("useNormalizedQuery - Refresh", () => {
	it("refetches when the daemon sends Refresh as its serialized string", async () => {
		const queryClient = new QueryClient({
			defaultOptions: { queries: { retry: false, gcTime: Infinity } },
		});
		queryClient.setQueryData(queryKey, { files: [] });

		handleResourceEvent(refresh(), options, queryKey, queryClient);
		await sleep(REFRESH_SETTLE_MS + 20);

		expect(queryClient.getQueryState(queryKey)?.isInvalidated).toBe(true);
	});

	it("fetches an active query once when every listener receives the same Refresh", async () => {
		const query = directoryQuery(20);
		await query.observer.refetch();
		const before = query.fetches();

		for (let listener = 0; listener < 5; listener++) {
			handleResourceEvent(refresh(), options, queryKey, query.queryClient);
		}
		await sleep(REFRESH_SETTLE_MS + 60);

		expect(query.fetches() - before).toBe(1);
		query.unsubscribe();
	});

	it("replaces a refetch that started before the Refresh", async () => {
		const query = directoryQuery(REFRESH_SETTLE_MS * 3);
		await query.observer.refetch();

		void query.observer.refetch();
		const stale = query.fetches();
		handleResourceEvent(refresh(), options, queryKey, query.queryClient);
		await sleep(REFRESH_SETTLE_MS * 5);

		expect(query.fetches()).toBe(stale + 1);
		expect(query.observer.getCurrentResult().data?.fetch).toBe(stale + 1);
		query.unsubscribe();
	});
});
