import "./setup";
import { describe, it, expect } from "bun:test";
import type { ReactNode } from "react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { renderHook, act } from "@testing-library/react";
import { SpacedriveClient } from "../../client";
import type { Transport } from "../../transport";
import type { Event } from "../../generated/types";
import { SpacedriveClientContext } from "../useClient";
import { useJobs } from "../useJobs";

function fakeTransport() {
	let subscribes = 0;
	let unsubscribes = 0;
	const listeners = new Set<(event: Event) => void>();
	const transport: Transport = {
		sendRequest: async () => ({ jobs: [] }),
		subscribe: async (callback) => {
			subscribes++;
			listeners.add(callback);
			return () => {
				unsubscribes++;
				listeners.delete(callback);
			};
		},
	};
	return {
		transport,
		emit: (event: Event) => listeners.forEach((listener) => listener(event)),
		counts: () => ({ subscribes, unsubscribes }),
	};
}

const progress = (n: number): Event => ({
	JobProgress: {
		job_id: "job-1",
		job_type: "reconcile",
		device_id: "device-1",
		progress: n / 100,
		message: null,
		generic_progress: null,
	},
});

describe("useJobs", () => {
	it("keeps one job event subscription while progress re-renders the caller", async () => {
		const { transport, emit, counts } = fakeTransport();
		const client = new SpacedriveClient(transport);
		const queryClient = new QueryClient();
		const wrapper = ({ children }: { children: ReactNode }) => (
			<QueryClientProvider client={queryClient}>
				<SpacedriveClientContext.Provider value={client}>
					{children}
				</SpacedriveClientContext.Provider>
			</QueryClientProvider>
		);

		const completed: string[] = [];
		const { rerender, unmount } = renderHook(
			({ tag }: { tag: string }) =>
				useJobs({ onJobCompleted: (jobId) => completed.push(`${tag}:${jobId}`) }),
			{ wrapper, initialProps: { tag: "first" } },
		);
		await act(async () => {});

		for (let n = 0; n < 50; n++) {
			await act(async () => emit(progress(n)));
			rerender({ tag: `render-${n}` });
		}
		rerender({ tag: "last" });

		expect(counts()).toEqual({ subscribes: 1, unsubscribes: 0 });

		await act(async () =>
			emit({
				JobCompleted: {
					job_id: "job-1",
					job_type: "reconcile",
					device_id: "device-1",
					output: { type: "Success" },
				},
			}),
		);
		expect(completed).toEqual(["last:job-1"]);

		unmount();
		expect(counts()).toEqual({ subscribes: 1, unsubscribes: 1 });
	});
});
