import { useEffect, useState } from "react";
import {
	useSpacedriveClient,
	type Event,
	type SdPath,
	type SpacedriveClient,
	type ThumbstripIdentity,
	type ThumbstripRequestInput,
	type ThumbstripRequestOutput,
} from "@sd/ts-client";
import { useServer } from "../../../contexts/ServerContext";

const completions = new Map<string, { epoch: number; ok: boolean }>();
const listeners = new Map<string, Set<(epoch: number, ok: boolean) => void>>();
let listening = false;

function identityKey(identity: {
	source_id: string;
	uuid: string;
	version: string;
}) {
	return `${identity.source_id}:${identity.uuid}:${identity.version}`;
}

function thumbstripResource(event: Event) {
	if (typeof event !== "object" || !("ResourceChanged" in event)) return null;
	const resource = event.ResourceChanged.resource;
	if (typeof resource !== "object" || resource === null || Array.isArray(resource)) {
		return null;
	}
	const { id, source_id, version, ok } = resource;
	if (
		typeof id !== "string" ||
		typeof source_id !== "string" ||
		typeof version !== "string"
	) {
		return null;
	}
	return { id, source_id, version, ok: ok === true };
}

function listen(client: SpacedriveClient) {
	if (listening) return;
	listening = true;
	client
		.subscribeFiltered({ resource_type: "thumbstrip" }, (event) => {
			const resource = thumbstripResource(event);
			if (!resource) return;
			const key = identityKey({
				source_id: resource.source_id,
				uuid: resource.id,
				version: resource.version,
			});
			const epoch = (completions.get(key)?.epoch ?? 0) + 1;
			const ok = resource.ok;
			completions.set(key, { epoch, ok });
			listeners.get(key)?.forEach((notify) => notify(epoch, ok));
		})
		.catch(() => {
			listening = false;
		});
}

/** Queue and observe a volume-scoped timeline sprite for one local video. */
export function useHotThumbstrip(path: SdPath, enabled: boolean) {
	const client = useSpacedriveClient();
	const { buildHotThumbstripUrl } = useServer();
	const [identity, setIdentity] = useState<ThumbstripIdentity | null>(null);
	const [ready, setReady] = useState(false);
	const [resolvedPath, setResolvedPath] = useState<string | null>(null);
	const pathKey =
		"Physical" in path
			? `${path.Physical.device_slug}:${path.Physical.path}`
			: null;

	useEffect(() => {
		if (!enabled || !pathKey) return;
		let active = true;
		client
			.execute<ThumbstripRequestInput, ThumbstripRequestOutput>(
				"action:thumbstrips.request.input",
				{ path },
			)
			.then((output) => {
				if (!active) return;
				const resolved = output.thumbstrip;
				setIdentity(resolved);
				setResolvedPath(pathKey);
				setReady(
					resolved?.ready === true ||
						(resolved
							? completions.get(identityKey(resolved))?.ok === true
							: false),
				);
			})
			.catch(() => {
				if (active) {
					setIdentity(null);
					setReady(false);
					setResolvedPath(pathKey);
				}
			});
		return () => {
			active = false;
		};
	}, [client, enabled, path, pathKey]);

	useEffect(() => {
		if (!enabled || !identity || identity.ready || !identity.pending) return;
		listen(client);
		const key = identityKey(identity);
		const notify = (_epoch: number, ok: boolean) => setReady(ok);
		const cells = listeners.get(key) ?? new Set();
		cells.add(notify);
		listeners.set(key, cells);
		return () => {
			const cells = listeners.get(key);
			cells?.delete(notify);
			if (cells && cells.size === 0) listeners.delete(key);
		};
	}, [client, enabled, identity]);

	if (!enabled || resolvedPath !== pathKey || !identity || !ready) {
		return { url: null, columns: 5, rows: 5 };
	}
	return {
		url: buildHotThumbstripUrl(
			identity.source_id,
			identity.uuid,
			identity.version,
		),
		columns: identity.columns,
		rows: identity.rows,
	};
}
