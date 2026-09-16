import { useEffect, useState } from "react";
import {
	useSpacedriveClient,
	type SdPath,
	type SpacedriveClient,
	type TileIdentity,
} from "@sd/ts-client";
import { useServer } from "../../../contexts/ServerContext";

/**
 * A tile from the daemon's thumbnail hot tier, for files that have no sidecar.
 *
 * Sidecars are keyed by content hash, which browsing an unindexed folder never
 * computes, so without this the grid falls back to loading full-resolution
 * originals into `img` tags. The hot tier is the daemon's answer: one 256px
 * tile per entry, baked once, read by every client.
 *
 * Two things are shared across every cell rather than done per cell. Identity
 * requests batch into one op per frame, which is exactly the set of cells the
 * grid decided to draw and the order it decided to draw them, and the daemon
 * bakes in that order. Bake completions come over a single subscription, since
 * a grid can hold hundreds of cells and each one waiting on its own listener
 * would rescan every batch.
 */

/** Identities already answered for, keyed by physical path. */
const identities = new Map<string, TileIdentity | null>();
/** Cells waiting on an identity, by path. */
const waiting = new Map<string, Set<(tile: TileIdentity | null) => void>>();
/** The address each waiting path came from, sent through as the caller gave
 * it so a file on another device is rejected by the daemon rather than baked
 * from a path that happens to exist here. */
const addresses = new Map<string, SdPath>();
/** The latest bake completion seen for each record survives cell unmounts. */
const bakeEpochs = new Map<string, number>();
/** Mounted cells waiting on a bake, by record uuid. */
const baking = new Map<string, Set<(epoch: number) => void>>();
let scheduled = false;
let listening = false;

function pathKey(path: SdPath): string | null {
	if (typeof path !== "object" || !("Physical" in path) || !path.Physical) {
		return null;
	}
	return path.Physical.path;
}

/** Ask for every path queued this frame, in the order the cells mounted. */
function flush(client: SpacedriveClient) {
	const batch = [...waiting.keys()].filter((key) => !identities.has(key));
	if (batch.length === 0) return;

	const paths = batch.map((key) => addresses.get(key)!);
	client
		.execute("action:thumbs.request.input", { paths })
		.then((output) => {
			const tiles = (output as { tiles: Array<TileIdentity | null> }).tiles;
			batch.forEach((key, index) => settle(key, tiles[index] ?? null));
		})
		.catch(() => {
			// The daemon is down, or too old for the op. Cells keep whatever
			// fallback they already have; a remount asks again.
			batch.forEach((key) => settle(key, null));
		});
}

function settle(key: string, tile: TileIdentity | null) {
	identities.set(key, tile);
	waiting.get(key)?.forEach((notify) => notify(tile));
	waiting.delete(key);
	addresses.delete(key);
}

/** One subscription for the whole grid, opened the first time a cell needs it. */
function listen(client: SpacedriveClient) {
	if (listening) return;
	listening = true;
	client
		.subscribeFiltered({ resource_type: "thumbnail" }, (event: any) => {
			const rows: Array<{ id?: string }> =
				event?.ResourceChangedBatch?.resources ??
				(event?.ResourceChanged?.resource ? [event.ResourceChanged.resource] : []);
			for (const row of rows) {
				if (!row?.id) continue;
				const epoch = (bakeEpochs.get(row.id) ?? 0) + 1;
				bakeEpochs.set(row.id, epoch);
				baking.get(row.id)?.forEach((notify) => notify(epoch));
			}
		})
		.catch(() => {
			// Without completions a cell that arrived before its bake keeps
			// its placeholder until it remounts, which is survivable.
			listening = false;
		});
}

/** A cell's hot tile: its URL once known, and whether one is still coming. */
export interface HotThumb {
	url: string | null;
	/** The daemon has not answered for this path yet. A caller must not reach
	 * for a full-resolution original meanwhile, which is the cost the hot tier
	 * exists to avoid. */
	pending: boolean;
}

/**
 * The hot-tier tile for `path`. Pass `enabled` false for cells that already
 * have a better image.
 */
export function useHotThumb(path: SdPath | null, enabled: boolean): HotThumb {
	const client = useSpacedriveClient();
	const { buildHotThumbUrl } = useServer();
	const key = path ? pathKey(path) : null;

	const [tile, setTile] = useState<TileIdentity | null>(() =>
		key ? (identities.get(key) ?? null) : null,
	);
	// Bumped when the daemon says this tile baked. The first load can land
	// before the bake does, and a 404 does not retry itself.
	const [baked, setBaked] = useState(() => {
		const cachedTile = key ? identities.get(key) : null;
		return cachedTile ? (bakeEpochs.get(cachedTile.uuid) ?? 0) : 0;
	});

	useEffect(() => {
		if (!enabled || !key) return;
		if (identities.has(key)) {
			setTile(identities.get(key) ?? null);
			return;
		}

		const notify = (resolved: TileIdentity | null) => setTile(resolved);
		addresses.set(key, path!);
		const cell = waiting.get(key) ?? new Set();
		cell.add(notify);
		waiting.set(key, cell);

		if (!scheduled) {
			scheduled = true;
			requestAnimationFrame(() => {
				scheduled = false;
				flush(client);
			});
		}

		return () => {
			waiting.get(key)?.delete(notify);
		};
	}, [client, enabled, key, path]);

	useEffect(() => {
		if (!enabled || !tile) return;
		listen(client);
		setBaked(bakeEpochs.get(tile.uuid) ?? 0);
		const notify = (epoch: number) => setBaked(epoch);
		const cell = baking.get(tile.uuid) ?? new Set();
		cell.add(notify);
		baking.set(tile.uuid, cell);
		return () => {
			const cell = baking.get(tile.uuid);
			cell?.delete(notify);
			if (cell && cell.size === 0) baking.delete(tile.uuid);
		};
	}, [client, enabled, tile]);

	if (!enabled || !key) return { url: null, pending: false };
	if (!tile) {
		return { url: null, pending: !identities.has(key) };
	}
	const url = buildHotThumbUrl(tile.source_id, tile.uuid, tile.version);
	return { url: url && baked > 0 ? `${url}?b=${baked}` : url, pending: false };
}
