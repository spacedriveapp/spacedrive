import "./setup";
import { describe, it, expect } from "bun:test";
import type { ReactNode } from "react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { renderHook, act } from "@testing-library/react";
import { SpacedriveClient } from "../../client";
import type { Transport } from "../../transport";
import type { Event } from "../../generated/types";
import { SpacedriveClientContext } from "../useClient";
import { JOB_HISTORY_LIMIT, JOB_REFETCH_COALESCE_MS, useJobs } from "../useJobs";

function fakeTransport() {
	let subscribes = 0;
	let unsubscribes = 0;
	const requests: any[] = [];
	const listeners = new Set<(event: Event) => void>();
	const transport: Transport = {
		sendRequest: async (request) => {
			requests.push(request);
			return { JsonOk: { jobs: [] } };
		},
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
		jobListRequests: () =>
			requests.filter((request) => JSON.stringify(request).includes("jobs.list")),
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

	it("asks for a bounded list and refetches once for a burst of job changes", async () => {
		const { transport, emit, jobListRequests } = fakeTransport();
		const client = new SpacedriveClient(transport);
		client.setCurrentLibrary("library-1", false);
		const queryClient = new QueryClient();
		const wrapper = ({ children }: { children: ReactNode }) => (
			<QueryClientProvider client={queryClient}>
				<SpacedriveClientContext.Provider value={client}>
					{children}
				</SpacedriveClientContext.Provider>
			</QueryClientProvider>
		);

		const { unmount } = renderHook(() => useJobs(), { wrapper });
		await act(() => new Promise((resolve) => setTimeout(resolve, 0)));
		expect(jobListRequests()).toHaveLength(1);
		expect(JSON.stringify(jobListRequests()[0])).toContain(
			`"limit":${JOB_HISTORY_LIMIT}`,
		);

		await act(async () => {
			for (let n = 0; n < 20; n++) {
				const job = { job_id: `job-${n}`, job_type: "proxy", device_id: "device-1" };
				emit({ JobStarted: job });
				emit({ JobCompleted: { ...job, output: { type: "Success" } } });
			}
		});
		await act(() => new Promise((resolve) => setTimeout(resolve, JOB_REFETCH_COALESCE_MS * 2)));

		expect(jobListRequests()).toHaveLength(2);
		unmount();
	});
});
