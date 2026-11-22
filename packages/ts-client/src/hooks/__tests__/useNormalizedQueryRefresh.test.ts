import { describe, expect, it } from "bun:test";
import { QueryClient } from "@tanstack/react-query";
import { handleResourceEvent } from "../useNormalizedQuery";

describe("useNormalizedQuery - Refresh", () => {
	it("refetches when the daemon sends Refresh as its serialized string", () => {
		const queryClient = new QueryClient({
			defaultOptions: { queries: { retry: false, gcTime: Infinity } },
		});
		const queryKey = ["query:files.directory_listing", "library", { path: "/a" }];
		queryClient.setQueryData(queryKey, { files: [] });

		handleResourceEvent(
			JSON.parse('"Refresh"'),
			{ query: "files.directory_listing", input: {}, resourceType: "file" },
			queryKey,
			queryClient,
		);

		expect(queryClient.getQueryState(queryKey)?.isInvalidated).toBe(true);
	});
});
