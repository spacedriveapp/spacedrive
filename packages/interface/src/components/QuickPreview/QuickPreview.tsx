import { useNormalizedQuery } from "../../contexts/SpacedriveContext";
import { usePlatform } from "../../contexts/PlatformContext";
import type { File } from "@sd/ts-client";
import { getContentKind } from "@sd/ts-client";
import { useEffect, useState } from "react";
import { formatBytes } from "../../routes/explorer/utils";
import { X } from "@phosphor-icons/react";
import { ContentRenderer } from "./ContentRenderer";

function MetadataPanel({ file }: { file: File }) {
	return (
		<div className="w-[280px] min-w-[280px] bg-sidebar-box border-l border-sidebar-line p-4 overflow-y-auto">
			<div className="space-y-4">
				<div>
					<div className="text-xs text-ink-dull mb-1">Name</div>
					<div className="text-sm text-ink break-words">
						{file.name}
					</div>
				</div>

				<div>
					<div className="text-xs text-ink-dull mb-1">Kind</div>
					<div className="text-sm text-ink capitalize">
						{getContentKind(file)}
					</div>
				</div>

				<div>
					<div className="text-xs text-ink-dull mb-1">Size</div>
					<div className="text-sm text-ink">
						{formatBytes(file.size || 0)}
					</div>
				</div>

				{file.extension && (
					<div>
						<div className="text-xs text-ink-dull mb-1">
							Extension
						</div>
						<div className="text-sm text-ink">{file.extension}</div>
					</div>
				)}

				{file.created_at && (
					<div>
						<div className="text-xs text-ink-dull mb-1">
							Created
						</div>
						<div className="text-sm text-ink">
							{new Date(file.created_at).toLocaleString()}
						</div>
					</div>
				)}

				{file.modified_at && (
					<div>
						<div className="text-xs text-ink-dull mb-1">
							Modified
						</div>
						<div className="text-sm text-ink">
							{new Date(file.modified_at).toLocaleString()}
						</div>
					</div>
				)}
			</div>
		</div>
	);
}

export function QuickPreview() {
	const platform = usePlatform();
	const [fileId, setFileId] = useState<string | null>(null);

	useEffect(() => {
		// Extract file_id from window label
		if (platform.getCurrentWindowLabel) {
			const label = platform.getCurrentWindowLabel();

			// Label format: "quick-preview-{file_id}"
			const match = label.match(/^quick-preview-(.+)$/);
			if (match) {
				setFileId(match[1]);
			}
		}
	}, [platform]);

	const {
		data: file,
		isLoading,
		error,
	} = useNormalizedQuery<{ file_id: string }, File>({
		wireMethod: "query:files.by_id",
		input: { file_id: fileId! },
		resourceType: "file",
		resourceId: fileId!,
		enabled: !!fileId,
	});

	const handleClose = () => {
		if (platform.closeCurrentWindow) {
			platform.closeCurrentWindow();
		}
	};

	// Keyboard shortcuts
	useEffect(() => {
		const handleKeyDown = (e: KeyboardEvent) => {
			if (e.code === "Escape") {
				handleClose();
			}
		};

		window.addEventListener("keydown", handleKeyDown);
		return () => window.removeEventListener("keydown", handleKeyDown);
	}, []);

	if (isLoading || !file) {
		return (
			<div className="h-screen flex items-center justify-center bg-app text-ink">
				<div className="animate-pulse">Loading...</div>
			</div>
		);
	}

	if (error) {
		return (
			<div className="h-screen flex items-center justify-center bg-app text-red-400">
				<div>
					<div className="text-lg font-medium mb-2">
						Error loading file
					</div>
					<div className="text-sm">{error.message}</div>
				</div>
			</div>
		);
	}

	return (
		<div className="h-screen flex flex-col bg-app text-ink">
			{/* Header */}
			<div className="flex items-center justify-between px-4 py-3 border-b border-app-line">
				<div className="text-sm font-medium truncate flex-1">
					{file.name}
				</div>
				<button
					onClick={handleClose}
					className="p-1 rounded-md hover:bg-app-hover text-ink-dull hover:text-ink"
				>
					<X size={16} weight="bold" />
				</button>
			</div>

			{/* Content Area */}
			<div className="flex-1 flex overflow-hidden">
				{/* File Content */}
				<div className="flex-1 p-6 bg-app-box/30">
					<ContentRenderer file={file} />
				</div>

				{/* Metadata Sidebar */}
				<MetadataPanel file={file} />
			</div>

			{/* Footer with keyboard hints */}
			<div className="px-4 py-2 border-t border-app-line bg-app-box/30">
				<div className="text-xs text-ink-dull text-center">
					Press <span className="text-ink">ESC</span> to close
				</div>
			</div>
		</div>
	);
}