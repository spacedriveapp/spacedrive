import { describe, expect, it } from "bun:test";
import { TauriTransport } from "../transport";

/** A Tauri event bus that delivers each emit to the listeners of its name only. */
function fakeTauri() {
	const listeners = new Map<string, Set<(event: any) => void>>();
	const invoked: { cmd: string; args: any }[] = [];
	const emit = (name: string, payload: unknown) =>
		listeners.get(name)?.forEach((handler) => handler({ payload }));
	const listen = async (name: string, handler: (event: any) => void) => {
		const set = listeners.get(name) ?? new Set();
		set.add(handler);
		listeners.set(name, set);
		return () => {
			set.delete(handler);
		};
	};
	const invoke = async (cmd: string, args?: any) => {
		invoked.push({ cmd, args });
		return invoked.length;
	};
	return { emit, listen, invoke, invoked };
}

describe("TauriTransport", () => {
	it("delivers each subscription only the events sent on its own channel", async () => {
		const tauri = fakeTauri();
		const transport = new TauriTransport(tauri.invoke, tauri.listen);
		const jobs: unknown[] = [];
		const files: unknown[] = [];

		await transport.subscribe((event) => jobs.push(event), {
			event_types: ["JobProgress"],
		});
		await transport.subscribe((event) => files.push(event), {
			event_types: ["ResourceChanged"],
			filter: { resource_type: "file" },
		});

		const [jobsChannel, filesChannel] = tauri.invoked.map(
			({ args }) => args.channel,
		);
		expect(jobsChannel).not.toBe(filesChannel);

		tauri.emit(jobsChannel, { JobProgress: { job_id: "a" } });
		tauri.emit(filesChannel, { ResourceChanged: { resource_type: "file" } });

		expect(jobs).toEqual([{ JobProgress: { job_id: "a" } }]);
		expect(files).toEqual([{ ResourceChanged: { resource_type: "file" } }]);
	});

	it("stops listening on its channel when unsubscribed", async () => {
		const tauri = fakeTauri();
		const transport = new TauriTransport(tauri.invoke, tauri.listen);
		const received: unknown[] = [];

		const unsubscribe = await transport.subscribe((event) =>
			received.push(event),
		);
		const channel = tauri.invoked[0].args.channel;
		await unsubscribe();
		tauri.emit(channel, "Refresh");

		expect(received).toEqual([]);
		expect(tauri.invoked.at(-1)?.cmd).toBe("unsubscribe_from_events");
	});
});
