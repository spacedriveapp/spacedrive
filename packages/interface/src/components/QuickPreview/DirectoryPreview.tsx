import { useMemo } from "react";
import type { File } from "@sd/ts-client";
import { File as FileComponent } from "../Explorer/File";
import { useNormalizedQuery } from "../../context";
import { Folder } from "@sd/assets/icons";

interface DirectoryPreviewProps {
	file: File;
}

export function DirectoryPreview({ file }: DirectoryPreviewProps) {
	const directoryQuery = useNormalizedQuery({
		wireMethod: "query:files.directory_listing",
		input: {
			path: file.sd_path,
			limit: null,
			include_hidden: false,
			sort_by: "modified" as any,
			folders_first: true,
		},
		resourceType: "file",
		pathScope: file.sd_path,
		enabled: true,
	});

	const allFiles = (directoryQuery.data as any)?.files || [];

	const directories = useMemo(() => {
		return allFiles;
	}, [allFiles]);

	const gridSize = 120;
	const gapSize = 12;

	if (directoryQuery.isLoading) {
		return (
			<div className="w-full h-full flex items-center justify-center">
				<div className="text-center">
					<img
						src={Folder}
						alt="Folder Icon"
						className="w-16 h-16 mb-4 mx-auto"
					/>
					<div className="text-lg font-medium text-ink">
						{file.name}
					</div>
					<div className="text-sm text-ink-dull mt-2">
						Loading directories...
					</div>
				</div>
			</div>
		);
	}

	if (directories.length === 0) {
		return (
			<div className="w-full h-full flex items-center justify-center">
				<div className="text-center">
					<img
						src={Folder}
						alt="Folder Icon"
						className="w-16 h-16 mb-4 mx-auto"
					/>
					<div className="text-lg font-medium text-ink">
						{file.name}
					</div>
					<div className="text-sm text-ink-dull mt-2">
						No subdirectories
					</div>
				</div>
			</div>
		);
	}

	const thumbSize = Math.max(gridSize * 0.6, 60);

	return (
		<div className="w-full h-full overflow-auto">
			<div
				className="grid p-6"
				style={{
					gridTemplateColumns: `repeat(auto-fill, minmax(${gridSize}px, 1fr))`,
					gridAutoRows: "max-content",
					gap: `${gapSize}px`,
				}}
			>
				{directories.map((dir) => (
					<div
						key={dir.id}
						className="flex flex-col items-center gap-2 p-1 rounded-lg hover:bg-app-hover/20"
					>
						<div className="rounded-lg p-2">
							<FileComponent.Thumb file={dir} size={thumbSize} />
						</div>
						<div className="w-full flex flex-col items-center">
							<div className="text-sm truncate px-2 py-0.5 rounded-md inline-block max-w-full text-ink">
								{dir.name}
							</div>
						</div>
					</div>
				))}
			</div>
		</div>
	);
}

