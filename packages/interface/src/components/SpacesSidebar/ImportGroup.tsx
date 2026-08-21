import { useState } from "react";
import { useNavigate } from "react-router-dom";
import {
	Camera,
	EnvelopeSimple,
	Note,
	SpinnerGap,
	Plus,
	type Icon,
} from "@phosphor-icons/react";
import {
	useLibraryQuery,
	useLibraryMutation,
} from "../../contexts/SpacedriveContext";
import { GroupHeader } from "./GroupHeader";

/**
 * "Your Mac" import affordances, visible from first launch so the empty
 * state is an invitation. Zero-config adapters import with one click;
 * everything else routes to the adapter gallery.
 */
type ImportTarget = {
	label: string;
	icon: Icon;
	/** Installed adapter id for one-click import, when it needs no config. */
	adapterId?: string;
	/** Shown when nothing can be imported yet. */
	soon?: boolean;
};

const TARGETS: ImportTarget[] = [
	{ label: "Photos", icon: Camera, adapterId: "apple-photos" },
	{ label: "Notes", icon: Note, adapterId: "apple-notes" },
	{ label: "Mail", icon: EnvelopeSimple, soon: true },
];

export function ImportGroup() {
	const navigate = useNavigate();
	const [isCollapsed, setCollapsed] = useState(false);
	const onToggle = () => setCollapsed((c) => !c);
	const [importing, setImporting] = useState<string | null>(null);

	const { data: sources, refetch } = useLibraryQuery({
		type: "sources.list",
		input: { data_type: null },
	});
	const createSource = useLibraryMutation("sources.create");
	const syncSource = useLibraryMutation("sources.sync");

	const existingAdapterIds = new Set(
		(sources ?? []).map((s: { adapter_id: string }) => s.adapter_id),
	);

	// Once every target is imported or pending, the group has done its job.
	const remaining = TARGETS.filter(
		(t) => !(t.adapterId && existingAdapterIds.has(t.adapterId)),
	);
	if (remaining.length === 0) return null;

	const runImport = async (target: ImportTarget) => {
		if (!target.adapterId || importing) return;
		setImporting(target.adapterId);
		try {
			const source = await createSource.mutateAsync({
				name: target.label,
				adapter_id: target.adapterId,
				config: {},
			});
			await syncSource.mutateAsync({
				source_id: (source as { id: string }).id,
			});
			await refetch();
		} catch (e) {
			console.error(`Import of ${target.label} failed:`, e);
		} finally {
			setImporting(null);
		}
	};

	return (
		<div>
			<GroupHeader label="Your Mac" isCollapsed={isCollapsed} onToggle={onToggle} />

			{!isCollapsed && (
				<div className="space-y-0.5">
					{remaining.map((target) => {
						const busy = importing === target.adapterId;
						const clickable = !!target.adapterId && !busy;
						return (
							<button
								key={target.label}
								disabled={!clickable}
								onClick={() =>
									target.adapterId
										? runImport(target)
										: navigate("/sources")
								}
								className={`group flex w-full items-center gap-2 rounded-md px-2 py-1 text-left text-sm font-medium ${
									clickable
										? "text-sidebar-inkDull hover:text-sidebar-ink"
										: "text-sidebar-inkDull/50"
								}`}
							>
								<target.icon className="size-4 shrink-0" />
								<span className="min-w-0 flex-1 truncate">
									{target.label}
								</span>
								{busy ? (
									<SpinnerGap className="size-3.5 shrink-0 animate-spin" />
								) : target.soon ? (
									<span className="shrink-0 text-[10px] uppercase tracking-wide text-sidebar-inkDull/60">
										soon
									</span>
								) : (
									<Plus className="size-3.5 shrink-0 opacity-0 group-hover:opacity-60" />
								)}
							</button>
						);
					})}
				</div>
			)}
		</div>
	);
}
