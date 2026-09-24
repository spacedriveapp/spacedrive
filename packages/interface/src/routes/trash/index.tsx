/**
 * The trash: what Spacedrive put there with a known location, newest
 * first, with restore through undo and emptying.
 */

import { ArrowCounterClockwise, Trash } from "@phosphor-icons/react";
import { Button, Switch, toast } from "@spacedrive/primitives";
import clsx from "clsx";
import { useState } from "react";
import type { TrashedItem } from "@sd/ts-client";
import { useUndoDialog } from "../../components/modals/UndoModal";
import { useLibraryMutation, useLibraryQuery } from "../../contexts/SpacedriveContext";
import { TopBarItem, TopBarPortal } from "../../TopBar";
import { formatBytes } from "../explorer/utils";

export function TrashView() {
	const [osTrash, setOsTrash] = useState(false);
	const [confirming, setConfirming] = useState(false);
	const openUndo = useUndoDialog();
	const { data, isLoading, refetch } = useLibraryQuery({
		type: "files.trash_list",
		input: { limit: 500 },
	});
	const empty = useLibraryMutation("files.trash_empty");
	const items = data?.items ?? [];

	const restore = (item: TrashedItem) =>
		openUndo({
			job: item.job,
			label: `trashing ${name(item)}`,
			effects: [item.sequence],
			onComplete: () => void refetch(),
		});

	const emptyTrash = async () => {
		try {
			const output = await empty.mutateAsync({ os_trash: osTrash });
			toast.success(
				`Removed ${output.purged + output.spacedrive_trash} items${
					output.os_trash_emptied ? " and emptied the platform's trash" : ""
				}`,
			);
			if (output.failed.length > 0) toast.error(output.failed.join("; "));
		} catch (error) {
			toast.error(error instanceof Error ? error.message : String(error));
		}
		setConfirming(false);
		void refetch();
	};

	return (
		<>
			<TopBarPortal
				center={
					<TopBarItem id="trash-title" label="Trash" priority="high">
						<div className="flex items-center gap-2">
							<Trash size={18} weight="bold" className="text-ink" />
							<span className="text-sm font-semibold text-ink">Trash</span>
						</div>
					</TopBarItem>
				}
			/>
			<div className="flex h-full flex-col overflow-hidden bg-app/80 pt-1.5">
				<div className="flex items-center gap-3 px-4 py-2">
					<div className="flex-1 text-xs text-ink-dull">
						{data
							? `${items.length} ${items.length === 1 ? "item" : "items"} Spacedrive trashed, newest first`
							: isLoading
								? "Reading the journals…"
								: ""}
					</div>
					{confirming ? (
						<div className="flex items-center gap-3 rounded-md bg-app-box px-3 py-1.5 text-xs">
							<label className="flex items-center gap-2 text-ink">
								<Switch checked={osTrash} onCheckedChange={setOsTrash} size="sm" />
								the platform's trash too
							</label>
							<Button type="button" size="sm" variant="outline" onClick={() => setConfirming(false)}>
								Cancel
							</Button>
							<Button
								type="button"
								size="sm"
								variant="colored"
								className="bg-red-500 text-white"
								disabled={empty.isPending}
								onClick={() => void emptyTrash()}
							>
								Remove for good
							</Button>
						</div>
					) : (
						<Button
							type="button"
							size="sm"
							variant="outline"
							disabled={items.length === 0}
							onClick={() => setConfirming(true)}
						>
							Empty the trash
						</Button>
					)}
				</div>

				<div className="flex-1 overflow-auto px-4 pb-4">
					{items.length === 0 && !isLoading && (
						<div className="rounded-lg border border-app-line bg-app-box/50 p-6 text-center text-sm text-ink-dull">
							Nothing Spacedrive trashed is waiting. What the platform's trash holds from
							elsewhere is not listed.
						</div>
					)}
					<div className="space-y-1">
						{items.map((item) => (
							<div
								key={`${item.job}-${item.sequence}`}
								className={clsx(
									"flex items-center gap-3 rounded-lg border border-app-line bg-app-box/50 px-3 py-2 text-xs",
									!item.present && "opacity-50",
								)}
							>
								<div className="min-w-0 flex-1">
									<div className="truncate text-ink" title={item.from}>
										{name(item)}
									</div>
									<div className="truncate text-ink-faint" title={item.location}>
										was in {parent(item.from)} · {new Date(item.trashed_at).toLocaleString()}
										{item.is_dir ? " · folder" : ` · ${formatBytes(item.size)}`}
										{item.spacedrive_trash ? " · Spacedrive trash" : ""}
										{item.present ? "" : " · no longer in the trash"}
									</div>
								</div>
								<Button
									type="button"
									size="sm"
									variant="outline"
									disabled={!item.present}
									onClick={() => restore(item)}
								>
									<ArrowCounterClockwise className="mr-1 size-3.5" weight="bold" />
									Put back
								</Button>
							</div>
						))}
					</div>
				</div>
			</div>
		</>
	);
}

function name(item: TrashedItem): string {
	const parts = item.from.split("/").filter(Boolean);
	return parts[parts.length - 1] ?? item.from;
}

function parent(path: string): string {
	const parts = path.split("/").filter(Boolean);
	parts.pop();
	return `/${parts.join("/")}`;
}
