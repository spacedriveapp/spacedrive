import { describe, expect, it } from "bun:test";
import { QueryClient, QueryObserver } from "@tanstack/react-query";
import { handleResourceEvent } from "../useNormalizedQuery";

const options = {
	query: "files.directory_listing",
	input: {},
	resourceType: "file",
};

describe("useNormalizedQuery - Refresh", () => {
	it("refetches when the daemon sends Refresh as its serialized string", () => {
		const queryClient = new QueryClient({
			defaultOptions: { queries: { retry: false, gcTime: Infinity } },
		});
		const queryKey = ["query:files.directory_listing", "library", { path: "/a" }];
		queryClient.setQueryData(queryKey, { files: [] });

		handleResourceEvent(JSON.parse('"Refresh"'), options, queryKey, queryClient);

		expect(queryClient.getQueryState(queryKey)?.isInvalidated).toBe(true);
	});

	it("fetches an active query once when every listener receives the same Refresh", async () => {
		const queryClient = new QueryClient({
			defaultOptions: { queries: { retry: false, gcTime: Infinity } },
		});
		const queryKey = ["query:files.directory_listing", "library", { path: "/a" }];
		let fetches = 0;
		const observer = new QueryObserver(queryClient, {
			queryKey,
			queryFn: async () => {
				fetches += 1;
				await new Promise((resolve) => setTimeout(resolve, 20));
				return { files: [] };
			},
		});
		const unsubscribe = observer.subscribe(() => {});
		await observer.refetch();
		fetches = 0;

		for (let listener = 0; listener < 5; listener++) {
			handleResourceEvent(JSON.parse('"Refresh"'), options, queryKey, queryClient);
		}
		await new Promise((resolve) => setTimeout(resolve, 50));

		expect(fetches).toBe(1);
		unsubscribe();
	});
});
