import { ArrowCounterClockwise } from "@phosphor-icons/react";
import type { JobListItem } from "../types";
import { getJobDisplayName } from "../types";
import { useUndoDialog } from "../../modals/UndoModal";

/** Whether a job left a journal with something to reverse. */
export function canUndo(job: JobListItem): boolean {
	return (
		(job.status === "completed" || job.status === "failed" || job.status === "cancelled") &&
		(job.journal?.reversible ?? 0) > 0
	);
}

/** Opens the undo dialog for a job that changed the filesystem. */
export function UndoButton({ job }: { job: JobListItem }) {
	const openUndo = useUndoDialog();
	if (!canUndo(job)) return null;
	return (
		<button
			type="button"
			onClick={(event) => {
				event.stopPropagation();
				openUndo({ job: job.id, label: getJobDisplayName(job) });
			}}
			className="flex-shrink-0 flex items-center justify-center w-4 h-4 rounded-full bg-app-hover hover:bg-app-selected transition-colors"
			title="Undo what this job did"
		>
			<ArrowCounterClockwise size={10} weight="bold" className="text-ink" />
		</button>
	);
}
