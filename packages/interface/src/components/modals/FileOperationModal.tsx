import { useEffect, useMemo, useState } from "react";
import { useForm } from "react-hook-form";
import {
	Files,
	FolderOpen,
	ArrowRight,
	Copy as CopyIcon,
	ArrowsLeftRight,
	FolderSimplePlus,
	Eye,
} from "@phosphor-icons/react";
import {
	Button,
	Dialog,
	dialogManager,
	toast,
	useDialog,
	type UseDialogProps,
} from "@spacedrive/primitives";
import type {
	FileConflictResolution,
	FileCopyInput,
	SdPath,
	File as FileType,
} from "@sd/ts-client";
import { RefusedError } from "@sd/ts-client";
import {
	useLibraryMutation,
	useLibraryPreview,
	useLibraryQuery,
	useLibraryValidate,
} from "../../contexts/SpacedriveContext";
import { File, FileStack } from "../../routes/explorer/File";
import { usePlanPreviewStore } from "../../routes/explorer/hooks/usePlanPreview";
import { useMergeFoldersDialog } from "./MergeFoldersModal";
import { hasErrors, pathName, PreflightPanel } from "./PreflightPanel";

interface FileOperationDialogProps {
	id: number;
	operation: "copy" | "move";
	sources: SdPath[];
	destination: SdPath;
	onComplete?: () => void;
}

/** The code copy validation gives a folder copied onto a folder of its name. */
const FOLDER_COLLISION = "copy.folder_collision";

/**
 * The copy and move dialog, with preflight: the daemon validates and previews
 * the exact input the dialog would dispatch, as its options change, and an
 * error finding gates the confirm button. A folder dropped on a same-name
 * folder is offered a merge beside copy and move.
 */
export function useFileOperationDialog() {
	return (options: Omit<FileOperationDialogProps, "id">) => {
		return dialogManager.create((props: UseDialogProps) => (
			<FileOperationDialog {...(props as FileOperationDialogProps)} {...options} />
		));
	};
}

function FileOperationDialog(props: FileOperationDialogProps) {
	const dialog = useDialog(props);
	const form = useForm();
	const [operation, setOperation] = useState<"copy" | "move">(props.operation);
	const [conflictResolution, setConflictResolution] =
		useState<FileConflictResolution>("Skip");
	const [failure, setFailure] = useState<string | null>(null);

	const input = useMemo<FileCopyInput>(
		() => ({
			sources: { paths: props.sources },
			destination: props.destination,
			overwrite: conflictResolution === "Overwrite",
			verify_checksum: false,
			preserve_timestamps: true,
			move_files: operation === "move",
			copy_method: "Auto",
			on_conflict: conflictResolution,
		}),
		[props.sources, props.destination, operation, conflictResolution],
	);

	const validation = useLibraryValidate({ type: "files.copy", input });
	const preview = useLibraryPreview({ type: "files.copy", input });
	const copyFiles = useLibraryMutation("files.copy");
	const openMergeFolders = useMergeFoldersDialog();
	const enterPreview = usePlanPreviewStore((state) => state.enter);

	// Fetch file info for sources (up to 3 for FileStack)
	const sourcePaths = props.sources
		.slice(0, 3)
		.map((s) => ("Physical" in s ? s.Physical.path : null))
		.filter((p): p is string => p !== null);

	const sourceFileQueries = sourcePaths.map((path) =>
		useLibraryQuery({ type: "files.by_path", input: { path } }, { enabled: !!path }),
	);

	const sourceFiles = sourceFileQueries
		.map((q) => q.data)
		.filter((f): f is FileType => f !== undefined && f !== null);

	const destPath: string | null =
		"Physical" in props.destination ? props.destination.Physical.path : null;

	const { data: destFile } = useLibraryQuery(
		{ type: "files.by_path", input: { path: destPath! } },
		{ enabled: !!destPath },
	);

	// A single folder copied onto a folder of its name can merge instead.
	const collision = validation.data?.findings.find(
		(finding) => finding.code === FOLDER_COLLISION,
	);
	const mergeInto = props.sources.length === 1 ? (collision?.path ?? null) : null;

	const refused = hasErrors(validation.data);
	const close = () => dialogManager.setState(props.id, { open: false });

	const handleSubmit = async () => {
		setFailure(null);
		try {
			await copyFiles.mutateAsync(input);
			close();
			props.onComplete?.();
		} catch (error) {
			if (error instanceof RefusedError) {
				setFailure(
					error.validation.findings.map((finding) => finding.message).join("; "),
				);
			} else {
				setFailure(error instanceof Error ? error.message : "Operation failed");
			}
		}
	};

	const handleMerge = () => {
		if (!mergeInto) return;
		close();
		openMergeFolders({
			sources: props.sources,
			destination: mergeInto,
			consume: operation === "move",
			onComplete: props.onComplete,
		});
	};

	const browse = () => {
		if (!preview.data) return;
		enterPreview({
			source: { type: "files.copy", input },
			plan: preview.data,
			label: `${operation === "copy" ? "Copy" : "Move"} ${props.sources.length} ${
				props.sources.length === 1 ? "item" : "items"
			} to ${pathName(props.destination)}`,
		});
		toast.info("Browsing the result; exit the preview from the bar above the files");
		close();
	};

	// Keyboard shortcuts
	useEffect(() => {
		const handleKeyDown = (e: KeyboardEvent) => {
			if (e.key === "Enter" && !e.shiftKey) {
				e.preventDefault();
				if (!refused && !copyFiles.isPending) void handleSubmit();
				return;
			}

			// Only handle other shortcuts if not typing in an input
			if ((e.target as HTMLElement)?.tagName === "INPUT") return;

			if ((e.metaKey || e.ctrlKey) && e.key === "1") {
				e.preventDefault();
				e.stopPropagation();
				setOperation("copy");
			}
			if ((e.metaKey || e.ctrlKey) && e.key === "2") {
				e.preventDefault();
				e.stopPropagation();
				setOperation("move");
			}
			if ((e.metaKey || e.ctrlKey) && e.key === "3" && mergeInto) {
				e.preventDefault();
				e.stopPropagation();
				handleMerge();
			}
			if (e.key === "s" && !e.metaKey && !e.ctrlKey) {
				e.preventDefault();
				setConflictResolution("Skip");
			}
			if (e.key === "k" && !e.metaKey && !e.ctrlKey) {
				e.preventDefault();
				setConflictResolution("AutoModifyName");
			}
			if (e.key === "o" && !e.metaKey && !e.ctrlKey) {
				e.preventDefault();
				setConflictResolution("Overwrite");
			}
		};

		window.addEventListener("keydown", handleKeyDown);
		return () => window.removeEventListener("keydown", handleKeyDown);
	});

	const sourceCount = props.sources.length;
	const pluralItems = sourceCount === 1 ? "item" : "items";
	const summary = preview.data?.summary;
	const verb = operation === "copy" ? "Copy" : "Move";
	const ctaLabel = summary
		? `${verb} ${summary.creates.files + summary.replaces.files + summary.moves.files} files`
		: verb;

	return (
		<Dialog
			dialog={dialog}
			form={form}
			title="File Operation"
			icon={<Files size={20} weight="bold" />}
			ctaLabel={ctaLabel}
			submitDisabled={refused || validation.isLoading || copyFiles.isPending}
			loading={copyFiles.isPending}
			onSubmit={form.handleSubmit(handleSubmit)}
			onCancelled={close}
			formClassName="!min-w-[480px] !max-w-[480px]"
		>
			<div className="space-y-5 py-2">
				{/* Source → Destination visual */}
				<div className="flex items-center gap-4">
					<div className="flex-1 flex flex-col items-center gap-2 min-w-0">
						{sourceFiles.length > 0 ? (
							<>
								{sourceFiles.length === 1 ? (
									<File.Thumb file={sourceFiles[0]} size={80} />
								) : (
									<FileStack files={sourceFiles} size={80} />
								)}
								<div className="text-center w-full">
									<div className="text-xs text-ink-dull mb-0.5">Source</div>
									{sourceFiles.length === 1 ? (
										<div className="text-sm font-medium text-ink truncate w-full">
											{sourceFiles[0].name}
										</div>
									) : (
										<div className="text-sm font-medium text-ink">
											{sourceCount} {pluralItems}
										</div>
									)}
								</div>
							</>
						) : (
							<>
								<Files className="size-20 text-ink-dull" weight="fill" />
								<div className="text-center">
									<div className="text-xs text-ink-dull mb-0.5">Source</div>
									<div className="text-sm font-medium text-ink">
										{sourceCount} {pluralItems}
									</div>
								</div>
							</>
						)}
					</div>

					<div className="flex-shrink-0">
						<ArrowRight className="size-6 text-accent" weight="bold" />
					</div>

					<div className="flex-1 flex flex-col items-center gap-2 min-w-0">
						{destFile ? (
							<>
								<File.Thumb file={destFile} size={80} />
								<div className="text-center w-full">
									<div className="text-xs text-ink-dull mb-0.5">To</div>
									<div className="text-sm font-medium text-ink truncate w-full">
										{destFile.name}
									</div>
								</div>
							</>
						) : (
							<>
								<FolderOpen className="size-20 text-accent" weight="fill" />
								<div className="text-center">
									<div className="text-xs text-ink-dull mb-0.5">To</div>
									<div className="text-sm font-medium text-ink truncate max-w-full">
										{pathName(props.destination)}
									</div>
								</div>
							</>
						)}
					</div>
				</div>

				{/* Operation type selection */}
				<div className="space-y-2">
					<div className="text-xs font-medium text-ink-dull mb-2">Operation:</div>
					<div className="flex gap-2">
						<OperationButton
							active={operation === "copy"}
							onClick={() => setOperation("copy")}
							icon={<CopyIcon className="size-4" weight="bold" />}
							label="Copy"
							shortcut="⌘1"
						/>
						<OperationButton
							active={operation === "move"}
							onClick={() => setOperation("move")}
							icon={<ArrowsLeftRight className="size-4" weight="bold" />}
							label="Move"
							shortcut="⌘2"
						/>
						{mergeInto && (
							<OperationButton
								active={false}
								onClick={handleMerge}
								icon={<FolderSimplePlus className="size-4" weight="bold" />}
								label="Merge"
								shortcut="⌘3"
							/>
						)}
					</div>
					{mergeInto && (
						<div className="text-[11px] text-ink-dull">
							A folder named {pathName(mergeInto)} is already there. Merge combines this
							one with it, file by file, after showing the plan.
						</div>
					)}
				</div>

				{/* Conflict resolution options */}
				<div className="space-y-2">
					<div className="text-xs font-medium text-ink-dull mb-2">
						If files already exist:
					</div>
					<div className="space-y-1">
						{(
							[
								{ value: "Skip", label: "Skip existing files", key: "S" },
								{ value: "AutoModifyName", label: "Keep both (rename new files)", key: "K" },
								{ value: "Overwrite", label: "Overwrite existing files", key: "O" },
							] as { value: FileConflictResolution; label: string; key: string }[]
						).map((option) => (
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
										onChange={() => setConflictResolution(option.value)}
										className="size-4 accent-accent cursor-pointer"
									/>
									<span className="text-sm text-ink">{option.label}</span>
								</div>
								<span className="text-xs text-ink-faint font-medium">{option.key}</span>
							</label>
						))}
					</div>
				</div>

				<PreflightPanel
					validation={validation.data}
					validating={validation.isFetching}
					plan={preview.data}
					planning={preview.isFetching}
					planError={preview.error}
				/>

				{failure && (
					<div className="rounded-md border border-red-500/20 bg-red-500/10 px-2.5 py-2 text-xs text-red-500">
						{failure}
					</div>
				)}

				<div className="flex justify-end">
					<Button
						type="button"
						variant="outline"
						size="sm"
						disabled={!preview.data}
						onClick={browse}
					>
						<Eye className="mr-1.5 size-4" weight="bold" />
						Browse the result
					</Button>
				</div>
			</div>
		</Dialog>
	);
}

function OperationButton({
	active,
	onClick,
	icon,
	label,
	shortcut,
}: {
	active: boolean;
	onClick: () => void;
	icon: React.ReactNode;
	label: string;
	shortcut: string;
}) {
	return (
		<button
			type="button"
			onClick={onClick}
			className={`flex-1 flex items-center justify-center gap-2 px-3 py-2 rounded-md text-sm font-medium transition-colors ${
				active ? "bg-accent text-white" : "bg-app-box text-ink hover:bg-app-hover"
			}`}
		>
			{icon}
			{label}
			<span className="text-xs opacity-60">{shortcut}</span>
		</button>
	);
}
