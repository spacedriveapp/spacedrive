import { useState, useEffect, useRef, useCallback } from "react";
import { Input } from "@sd/ui";
import type { File } from "@sd/ts-client";
import clsx from "clsx";

interface InlineNameEditProps {
	file: File;
	onSave: (newName: string) => Promise<void>;
	onCancel: () => void;
	className?: string;
}

/**
 * Inline file name editing component.
 *
 * - Auto-focuses and selects text on mount
 * - Only edits the name portion (excludes extension for files)
 * - Handles Enter (save), Escape (cancel), and blur (cancel)
 */
export function InlineNameEdit({ file, onSave, onCancel, className }: InlineNameEditProps) {
	// For files with extensions, only edit the name part
	const hasExtension = file.extension && file.extension.length > 0;
	const nameWithoutExtension = file.name;

	const [value, setValue] = useState(nameWithoutExtension);
	const [isSaving, setIsSaving] = useState(false);
	const inputRef = useRef<HTMLInputElement>(null);

	// Auto-focus and select on mount
	useEffect(() => {
		if (inputRef.current) {
			inputRef.current.focus();
			inputRef.current.select();
		}
	}, []);

	const handleSave = useCallback(async () => {
		if (isSaving) return;

		const trimmedValue = value.trim();

		// Cancel if empty
		if (!trimmedValue) {
			onCancel();
			return;
		}

		// Construct full name with extension
		const fullNewName = hasExtension ? `${trimmedValue}.${file.extension}` : trimmedValue;
		const currentFullName = hasExtension ? `${file.name}.${file.extension}` : file.name;

		// If unchanged, just cancel (no mutation needed)
		if (fullNewName === currentFullName) {
			onCancel();
			return;
		}

	setIsSaving(true);
	try {
		await onSave(fullNewName);
	} catch (error) {
		setIsSaving(false);
		// Keep in edit mode on error - let parent handle error display
	}
	}, [value, isSaving, hasExtension, file.extension, file.name, onSave, onCancel]);

	const handleKeyDown = useCallback((e: React.KeyboardEvent) => {
		if (e.key === "Enter") {
			e.preventDefault();
			e.stopPropagation();
			handleSave();
		} else if (e.key === "Escape") {
			e.preventDefault();
			e.stopPropagation();
			onCancel();
		}
	}, [handleSave, onCancel]);

	const handleBlur = useCallback(() => {
		// Cancel on blur (don't save - following macOS Finder behavior)
		if (!isSaving) {
			onCancel();
		}
	}, [isSaving, onCancel]);

	return (
		<div className={clsx("inline-flex items-center", className)}>
			<Input
				ref={inputRef}
				value={value}
				onChange={(e) => setValue(e.target.value)}
				onKeyDown={handleKeyDown}
				onBlur={handleBlur}
				variant="transparent"
				size="xs"
				disabled={isSaving}
				className={clsx(
					"min-w-[60px] !h-auto !py-0.5 !px-1 text-center",
					isSaving && "opacity-50"
				)}
				inputElementClassName="text-center"
			/>
			{hasExtension && (
				<span className="text-ink-dull text-sm">.{file.extension}</span>
			)}
		</div>
	);
}
