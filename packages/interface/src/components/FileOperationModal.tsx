import { useState } from "react";
import { useForm } from "react-hook-form";
import {
	Files,
	FolderOpen,
	Warning,
	CheckCircle,
	CircleNotch,
} from "@phosphor-icons/react";
import {
	Dialog,
	dialogManager,
	useDialog,
} from "@sd/ui";
import type { SdPath } from "@sd/ts-client";
import { useLibraryMutation } from "../context";
import { sounds } from "@sd/assets/sounds";

interface FileOperationDialogProps {
	id: number;
	operation: "copy" | "move";
	sources: SdPath[];
	destination: SdPath;
	onComplete?: () => void;
}

type ConflictResolution = "Overwrite" | "AutoModifyName" | "Skip" | "Abort";

type DialogPhase =
	| { type: "form" }
	| { type: "executing" }
	| { type: "error"; message: string };

export function useFileOperationDialog() {
	return (options: Omit<FileOperationDialogProps, "id">) => {
		return dialogManager.create((props: FileOperationDialogProps) => (
			<FileOperationDialog {...props} {...options} />
		));
	};
}

function FileOperationDialog(props: FileOperationDialogProps) {
	const dialog = useDialog(props);
	const form = useForm();
	const [phase, setPhase] = useState<DialogPhase>({ type: "form" });
	const [conflictResolution, setConflictResolution] = useState<ConflictResolution>("Skip");

	const copyFiles = useLibraryMutation("files.copy");

	const handleSubmit = async () => {
		try {
			setPhase({ type: "executing" });

			// Execute with the user's chosen conflict resolution
			await copyFiles.mutateAsync({
				sources: { paths: props.sources },
				destination: props.destination,
				overwrite: conflictResolution === "Overwrite",
				verify_checksum: false,
				preserve_timestamps: true,
				move_files: props.operation === "move",
				copy_method: "Auto",
				on_conflict: conflictResolution,
			});

			// Play completion sound
			sounds.copy();

			// Close immediately on success
			dialogManager.setState(props.id, { open: false });
			props.onComplete?.();
		} catch (error) {
			setPhase({
				type: "error",
				message: error instanceof Error ? error.message : "Operation failed",
			});
		}
	};

	const handleCancel = () => {
		dialogManager.setState(props.id, { open: false });
	};

	// Executing state
	if (phase.type === "executing") {
		return (
			<Dialog
				dialog={dialog}
				form={form}
				title={props.operation === "copy" ? "Copying Files" : "Moving Files"}
				icon={<Files size={20} weight="bold" />}
				hideButtons
			>
				<div className="space-y-3 py-6">
					<div className="flex items-center justify-center gap-3">
						<CircleNotch className="size-6 text-accent animate-spin" weight="bold" />
						<span className="text-sm text-ink">
							{props.operation === "copy" ? "Copying files..." : "Moving files..."}
						</span>
					</div>
				</div>
			</Dialog>
		);
	}

	// Error state
	if (phase.type === "error") {
		return (
			<Dialog
				dialog={dialog}
				form={form}
				title="Operation Failed"
				icon={<Warning size={20} weight="fill" className="text-red-500" />}
				ctaLabel="Close"
				onSubmit={handleCancel}
			>
				<div className="flex flex-col gap-4 py-4">
					<div className="flex items-start gap-2 p-3 bg-red-500/10 border border-red-500/20 rounded-md">
						<Warning className="size-5 text-red-500 mt-0.5" weight="fill" />
						<div className="flex-1">
							<div className="text-sm font-medium text-ink mb-1">Error</div>
							<div className="text-xs text-ink-dull">{phase.message}</div>
						</div>
					</div>
				</div>
			</Dialog>
		);
	}

	// Form state - let user choose conflict resolution
	return (
		<Dialog
			dialog={dialog}
			form={form}
			title={props.operation === "copy" ? "Copy Files" : "Move Files"}
			icon={<Files size={20} weight="bold" />}
			ctaLabel={props.operation === "copy" ? "Copy" : "Move"}
			onSubmit={handleSubmit}
			onCancelled={handleCancel}
		>
			<div className="space-y-4 py-2">
				{/* Destination info */}
				<div className="flex items-start gap-3 p-3 bg-app rounded-md">
					<FolderOpen className="size-5 text-accent mt-0.5" weight="fill" />
					<div className="flex-1 min-w-0">
						<div className="text-xs text-ink-dull mb-1">
							{props.operation === "copy" ? "Copying to:" : "Moving to:"}
						</div>
						<div className="text-sm text-ink font-medium truncate">
							{formatDestination(props.destination)}
						</div>
						<div className="text-xs text-ink-faint mt-1">
							{props.sources.length} {props.sources.length === 1 ? "item" : "items"}
						</div>
					</div>
				</div>

				{/* Conflict resolution options */}
				<div className="space-y-2">
					<div className="text-xs font-medium text-ink-dull mb-2">
						If files already exist:
					</div>
					{[
						{ value: "Skip", label: "Skip existing files" },
						{ value: "AutoModifyName", label: "Keep both (rename new files)" },
						{ value: "Overwrite", label: "Overwrite existing files" },
					].map((option) => (
						<label
							key={option.value}
							className="flex items-center gap-2 p-2 rounded-md hover:bg-app-hover cursor-pointer"
						>
							<input
								type="radio"
								name="conflict-resolution"
								value={option.value}
								checked={conflictResolution === option.value}
								onChange={() => setConflictResolution(option.value as ConflictResolution)}
								className="size-4 accent-accent cursor-pointer"
							/>
							<span className="text-sm text-ink">{option.label}</span>
						</label>
					))}
				</div>
			</div>
		</Dialog>
	);
}

// Utility functions
function formatDestination(path: SdPath): string {
	if (!path || typeof path !== "object") {
		return "Unknown";
	}

	if ("Physical" in path && path.Physical) {
		return path.Physical.path || "Unknown";
	}

	if ("Cloud" in path && path.Cloud) {
		return path.Cloud.path || "Unknown";
	}

	if ("Content" in path && path.Content) {
		return `Content: ${path.Content.content_id}`;
	}

	if ("Sidecar" in path && path.Sidecar) {
		return `Sidecar: ${path.Sidecar.entry_id}`;
	}

	return "Unknown";
}
