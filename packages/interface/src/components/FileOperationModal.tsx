import { useState, useEffect } from "react";
import { useForm } from "react-hook-form";
import {
	Files,
	FolderOpen,
	Warning,
	CheckCircle,
	CircleNotch,
	ArrowRight,
	Copy as CopyIcon,
	ArrowsLeftRight,
} from "@phosphor-icons/react";
import {
	Dialog,
	dialogManager,
	useDialog,
} from "@sd/ui";
import type { SdPath } from "@sd/ts-client";
import { useLibraryMutation } from "../context";
import { sounds } from "@sd/assets/sounds";
import { File } from "./Explorer/File";

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
	const [operation, setOperation] = useState<"copy" | "move">(props.operation);
	const [conflictResolution, setConflictResolution] = useState<ConflictResolution>("Skip");

	const copyFiles = useLibraryMutation("files.copy");

	// Check if any source is the same as destination
	const hasSameSourceDest = props.sources.some((source) => {
		if ("Physical" in source && "Physical" in props.destination) {
			return source.Physical.path === props.destination.Physical.path;
		}
		return false;
	});

	// Auto-close if invalid operation (must be in useEffect to avoid render loop)
	useEffect(() => {
		if (hasSameSourceDest) {
			dialogManager.setState(props.id, { open: false });
		}
	}, [hasSameSourceDest, props.id]);

	if (hasSameSourceDest) {
		return null;
	}

	const handleSubmit = async () => {
		try {
			setPhase({ type: "executing" });

			// Execute with the user's chosen operation and conflict resolution
			await copyFiles.mutateAsync({
				sources: { paths: props.sources },
				destination: props.destination,
				overwrite: conflictResolution === "Overwrite",
				verify_checksum: false,
				preserve_timestamps: true,
				move_files: operation === "move",
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

	// Keyboard shortcuts
	useEffect(() => {
		if (phase.type !== "form") return;

		const handleKeyDown = (e: KeyboardEvent) => {
			// Enter - Submit
			if (e.key === "Enter" && !e.shiftKey) {
				e.preventDefault();
				handleSubmit();
				return;
			}

			// Only handle other shortcuts if not typing in an input
			if ((e.target as HTMLElement)?.tagName === "INPUT") return;

			// ⌘1 / Ctrl+1 - Copy mode
			if ((e.metaKey || e.ctrlKey) && e.key === "1") {
				e.preventDefault();
				e.stopPropagation();
				setOperation("copy");
			}
			// ⌘2 / Ctrl+2 - Move mode
			if ((e.metaKey || e.ctrlKey) && e.key === "2") {
				e.preventDefault();
				e.stopPropagation();
				setOperation("move");
			}
			// S - Skip
			if (e.key === "s" && !e.metaKey && !e.ctrlKey) {
				e.preventDefault();
				setConflictResolution("Skip");
			}
			// K - Keep both
			if (e.key === "k" && !e.metaKey && !e.ctrlKey) {
				e.preventDefault();
				setConflictResolution("AutoModifyName");
			}
			// O - Overwrite
			if (e.key === "o" && !e.metaKey && !e.ctrlKey) {
				e.preventDefault();
				setConflictResolution("Overwrite");
			}
		};

		window.addEventListener("keydown", handleKeyDown);
		return () => window.removeEventListener("keydown", handleKeyDown);
	}, [phase.type, operation, conflictResolution]);

	// Executing state
	if (phase.type === "executing") {
		return (
			<Dialog
				dialog={dialog}
				form={form}
				title={operation === "copy" ? "Copying Files" : "Moving Files"}
				icon={<Files size={20} weight="bold" />}
				hideButtons
			>
				<div className="space-y-3 py-6">
					<div className="flex items-center justify-center gap-3">
						<CircleNotch className="size-6 text-accent animate-spin" weight="bold" />
						<span className="text-sm text-ink">
							{operation === "copy" ? "Copying files..." : "Moving files..."}
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

	const sourceCount = props.sources.length;
	const pluralItems = sourceCount === 1 ? "item" : "items";

	// Form state - let user choose operation and conflict resolution
	return (
		<Dialog
			dialog={dialog}
			form={form}
			title="File Operation"
			icon={<Files size={20} weight="bold" />}
			ctaLabel={operation === "copy" ? "Copy" : "Move"}
			onSubmit={handleSubmit}
			onCancelled={handleCancel}
		>
			<div className="space-y-5 py-2">
				{/* Source → Destination visual */}
				<div className="flex items-center gap-4">
					{/* Source */}
					<div className="flex-1 flex flex-col items-center gap-2 p-3 bg-app rounded-lg">
						<Files className="size-8 text-ink-dull" weight="fill" />
						<div className="text-center">
							<div className="text-xs text-ink-dull mb-0.5">From</div>
							<div className="text-sm font-medium text-ink">
								{sourceCount} {pluralItems}
							</div>
							{sourceCount === 1 && (
								<div className="text-xs text-ink-faint mt-1 truncate max-w-full">
									{getFileName(props.sources[0])}
								</div>
							)}
						</div>
					</div>

					{/* Arrow */}
					<div className="flex-shrink-0">
						<ArrowRight className="size-6 text-accent" weight="bold" />
					</div>

					{/* Destination */}
					<div className="flex-1 flex flex-col items-center gap-2 p-3 bg-app rounded-lg">
						<FolderOpen className="size-8 text-accent" weight="fill" />
						<div className="text-center">
							<div className="text-xs text-ink-dull mb-0.5">To</div>
							<div className="text-sm font-medium text-ink truncate max-w-full">
								{getFileName(props.destination)}
							</div>
						</div>
					</div>
				</div>

				{/* Operation type selection */}
				<div className="space-y-2">
					<div className="text-xs font-medium text-ink-dull mb-2">
						Operation:
					</div>
					<div className="flex gap-2">
						<button
							type="button"
							onClick={() => setOperation("copy")}
							className={`flex-1 flex items-center justify-center gap-2 px-3 py-2 rounded-md text-sm font-medium transition-colors ${
								operation === "copy"
									? "bg-accent text-white"
									: "bg-app-box text-ink hover:bg-app-hover"
							}`}
						>
							<CopyIcon className="size-4" weight="bold" />
							Copy
							<span className="text-xs opacity-60">⌘1</span>
						</button>
						<button
							type="button"
							onClick={() => setOperation("move")}
							className={`flex-1 flex items-center justify-center gap-2 px-3 py-2 rounded-md text-sm font-medium transition-colors ${
								operation === "move"
									? "bg-accent text-white"
									: "bg-app-box text-ink hover:bg-app-hover"
							}`}
						>
							<ArrowsLeftRight className="size-4" weight="bold" />
							Move
							<span className="text-xs opacity-60">⌘2</span>
						</button>
					</div>
				</div>

				{/* Conflict resolution options */}
				<div className="space-y-2">
					<div className="text-xs font-medium text-ink-dull mb-2">
						If files already exist:
					</div>
					<div className="space-y-1">
						{[
							{ value: "Skip", label: "Skip existing files", key: "S" },
							{ value: "AutoModifyName", label: "Keep both (rename new files)", key: "K" },
							{ value: "Overwrite", label: "Overwrite existing files", key: "O" },
						].map((option) => (
							<label
								key={option.value}
								className="flex items-center justify-between gap-2 px-2 py-2 rounded-md hover:bg-app-hover cursor-pointer transition-colors"
							>
								<div className="flex items-center gap-2">
									<input
										type="radio"
										name="conflict-resolution"
										value={option.value}
										checked={conflictResolution === option.value}
										onChange={() => setConflictResolution(option.value as ConflictResolution)}
										className="size-4 accent-accent cursor-pointer"
									/>
									<span className="text-sm text-ink">{option.label}</span>
								</div>
								<span className="text-xs text-ink-faint font-medium">{option.key}</span>
							</label>
						))}
					</div>
				</div>
			</div>
		</Dialog>
	);
}

// Utility functions
function getFileName(path: SdPath): string {
	if (!path || typeof path !== "object") {
		return "Unknown";
	}

	if ("Physical" in path && path.Physical) {
		const pathStr = path.Physical.path || "";
		const parts = pathStr.split("/");
		return parts[parts.length - 1] || pathStr;
	}

	if ("Cloud" in path && path.Cloud) {
		const pathStr = path.Cloud.path || "";
		const parts = pathStr.split("/");
		return parts[parts.length - 1] || pathStr;
	}

	return "Unknown";
}

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
