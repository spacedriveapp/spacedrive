import type { File as FileType } from "@sd/ts-client";
import { File } from "./File";

interface FileStackProps {
	files: FileType[];
	size?: number;
}

/**
 * FileStack - Renders multiple files stacked with rotation
 * Shows up to 3 files stacked on top of each other with slight rotation
 */
export function FileStack({ files, size = 64 }: FileStackProps) {
	const displayFiles = files.slice(0, 3);
	const remainingCount = Math.max(0, files.length - 3);

	// Rotation angles for visual stacking effect
	const rotations = [-4, 0, 4];

	return (
		<div className="relative" style={{ width: size, height: size }}>
			{displayFiles.map((file, index) => (
				<div
					key={file.id}
					className="absolute inset-0 transition-transform"
					style={{
						transform: `rotate(${rotations[index]}deg) translateY(${index * -2}px)`,
						zIndex: index,
					}}
				>
					<File.Thumb file={file} size={size} />
				</div>
			))}

			{/* Show count badge if more than 3 files */}
			{remainingCount > 0 && (
				<div className="absolute -bottom-1 -right-1 size-6 rounded-full bg-accent text-white text-xs font-bold flex items-center justify-center shadow-lg border-2 border-app z-10">
					+{remainingCount}
				</div>
			)}
		</div>
	);
}
