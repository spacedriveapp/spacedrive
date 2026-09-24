import { create } from "zustand";
import type {
	ChangeKind,
	FsPlan,
	LibraryPreview,
	OverlaidRow,
	SpacedriveClient,
} from "@sd/ts-client";
import { WIRE_METHODS } from "@sd/ts-client";

/** A preview call: the action and the input it takes. */
export type PreviewCall = {
	[K in LibraryPreview["type"]]: {
		type: K;
		input: Extract<LibraryPreview, { type: K }>["input"];
	};
}[LibraryPreview["type"]];

/**
 * The plan the explorer is browsing through, if any.
 *
 * In preview mode every directory listing passes the plan's handle, and
 * comes back as the directory after the operation, with a change per row
 * the plan touches. The daemon keeps the handle for a while; when it lapses
 * the preview is rebuilt from the same input, so the mode keeps
 * what produced the plan as well as the plan.
 */
export interface PlanPreview {
	/** The preview call that produced the plan, for rebuilding it. */
	source: PreviewCall;
	plan: FsPlan;
	/** A line for the banner: what the operation is. */
	label: string;
}

interface PlanPreviewStore {
	preview: PlanPreview | null;
	/** What the plan does to rows of the directory on screen, by full name. */
	rows: Map<string, ChangeKind>;
	enter: (preview: PlanPreview) => void;
	leave: () => void;
	setRows: (rows: OverlaidRow[]) => void;
	/** Preview again from the same input, for a handle that lapsed. */
	rebuild: (client: SpacedriveClient) => Promise<void>;
}

export const usePlanPreviewStore = create<PlanPreviewStore>((set, get) => ({
	preview: null,
	rows: new Map(),
	enter: (preview) => set({ preview, rows: new Map() }),
	leave: () => set({ preview: null, rows: new Map() }),
	setRows: (rows) => set({ rows: new Map(rows.map((row) => [row.name, row.change])) }),
	rebuild: async (client) => {
		const current = get().preview;
		if (!current) return;
		const plan = await client.execute<PreviewCall["input"], FsPlan>(
			WIRE_METHODS.libraryPreviews[current.source.type],
			current.source.input,
		);
		// Only replace what is still being browsed.
		if (get().preview === current) {
			set({ preview: { ...current, plan } });
		}
	},
}));

export function usePlanPreview() {
	return usePlanPreviewStore((state) => state.preview);
}

/** The full name a listing row is keyed by in an overlay. */
export function rowName(file: { name: string; extension: string | null }): string {
	return file.extension ? `${file.name}.${file.extension}` : file.name;
}

/** What the browsed plan does to this row, or undefined outside preview mode. */
export function useOverlaidChange(file: {
	name: string;
	extension: string | null;
}): ChangeKind | undefined {
	return usePlanPreviewStore((state) =>
		state.preview ? state.rows.get(rowName(file)) : undefined,
	);
}

/** Whether a lapsed handle is what a listing error says. */
export function isLapsedHandle(error: unknown): boolean {
	return error instanceof Error && error.message.includes("lapsed");
}
