import { useState } from "react";
import { FolderOpen } from "@phosphor-icons/react";
import { Button, Input, Label } from "@spacedrive/primitives";
import type { Volume } from "@sd/ts-client";
import clsx from "clsx";
import { useLibraryQuery } from "../../../contexts/SpacedriveContext";
import { usePlatform } from "../../../contexts/PlatformContext";
import { inclusionOf } from "./addToLibrary";

import FolderIcon from "@sd/assets/icons/Folder.webp";
import DriveIcon from "@sd/assets/icons/Drive.webp";
import HDDIcon from "@sd/assets/icons/HDD.webp";
import ServerIcon from "@sd/assets/icons/Server.webp";

/**
 * The steps that choose a scope when Add to Library opens without one, from
 * Home. Every choice lands in the same setup step a folder or drive picked in
 * the Explorer does.
 */

export type StorageCategory = "local" | "cloud" | "network" | "external";

const categories: {
	id: StorageCategory;
	label: string;
	description: string;
	icon: string;
}[] = [
	{
		id: "local",
		label: "Local Folder",
		description: "Add a folder on this computer",
		icon: FolderIcon,
	},
	{
		id: "cloud",
		label: "Cloud Storage",
		description: "Connect S3, Google Drive, Dropbox, etc.",
		icon: DriveIcon,
	},
	{
		id: "network",
		label: "Network Protocol",
		description: "SMB, NFS, SFTP, WebDAV",
		icon: ServerIcon,
	},
	{
		id: "external",
		label: "External Drive",
		description: "Add a connected drive",
		icon: HDDIcon,
	},
];

export function CategoryGrid({
	onSelect,
}: {
	onSelect: (category: StorageCategory) => void;
}) {
	return (
		<div className="grid grid-cols-2 gap-3">
			{categories.map((category) => (
				<button
					key={category.id}
					type="button"
					onClick={() => onSelect(category.id)}
					className={clsx(
						"flex flex-col items-center gap-3 rounded-lg border p-6",
						"transition-all hover:scale-[1.02]",
						"border-app-line bg-app-box hover:border-accent/50 hover:bg-app-hover",
					)}
				>
					<img src={category.icon} className="size-12" alt="" />
					<div className="text-center">
						<div className="text-sm font-medium text-ink">{category.label}</div>
						<div className="mt-1 text-xs text-ink-faint">{category.description}</div>
					</div>
				</button>
			))}
		</div>
	);
}

/** Browse, type, or pick a suggested folder; any of them selects the path. */
export function FolderPicker({ onPick }: { onPick: (path: string) => void }) {
	const platform = usePlatform();
	const [typed, setTyped] = useState("");
	const { data: systemFolders } = useLibraryQuery({
		type: "paths.system_folders",
		input: null,
	});
	const { data: sources } = useLibraryQuery({
		type: "sources.list",
		input: { data_type: null },
	});
	// A folder already inside a source is kept; suggesting it would only
	// nest a second source under the first.
	const suggested = (systemFolders?.folders ?? []).filter((folder) => {
		if (!("Physical" in folder.sd_path)) return true;
		const inclusion = inclusionOf(folder.sd_path.Physical.path, sources ?? []);
		return !inclusion.exact && !inclusion.enclosing;
	});

	const browse = async () => {
		if (!platform.openDirectoryPickerDialog) return;
		const selected = await platform.openDirectoryPickerDialog({
			title: "Choose a folder to add",
			multiple: false,
		});
		if (selected && typeof selected === "string") onPick(selected);
	};

	const submitTyped = () => {
		const path = typed.trim();
		if (path) onPick(path);
	};

	return (
		<div className="space-y-4">
			{platform.openDirectoryPickerDialog && (
				<button
					type="button"
					onClick={browse}
					className={clsx(
						"flex w-full items-center gap-3 rounded-lg border border-dashed p-4",
						"border-app-line bg-app-box hover:border-accent/50 hover:bg-app-hover",
					)}
				>
					<FolderOpen size={24} className="text-accent" weight="fill" />
					<div className="text-left">
						<div className="text-sm font-medium text-ink">Browse for a folder</div>
						<div className="text-xs text-ink-faint">Choose any folder on this computer</div>
					</div>
				</button>
			)}

			<div className="space-y-2">
				<Label>Folder path</Label>
				<div className="flex gap-2">
					<Input
						size="md"
						className="flex-1 bg-app-input"
						placeholder="/path/to/folder"
						value={typed}
						onChange={(event) => setTyped(event.target.value)}
						onKeyDown={(event) => {
							if (event.key === "Enter") {
								event.preventDefault();
								submitTyped();
							}
						}}
					/>
					<Button
						type="button"
						variant="accent"
						size="md"
						disabled={!typed.trim()}
						onClick={submitTyped}
					>
						Continue
					</Button>
				</div>
			</div>

			{suggested.length > 0 && (
				<div className="space-y-2">
					<Label>Suggested</Label>
					<div className="max-h-[260px] space-y-1 overflow-y-auto pr-1">
						{suggested.map((folder) => (
							<button
								key={folder.path}
								type="button"
								onClick={() => onPick(folder.path)}
								className={clsx(
									"flex w-full items-center gap-3 rounded-lg border p-2.5 text-left",
									"border-app-line bg-app-box hover:border-accent/50 hover:bg-app-hover",
								)}
							>
								<img src={FolderIcon} className="size-7" alt="" />
								<div className="min-w-0 flex-1">
									<div className="truncate text-sm text-ink">{folder.name}</div>
									<div className="truncate text-xs text-ink-faint">{folder.path}</div>
								</div>
							</button>
						))}
					</div>
				</div>
			)}
		</div>
	);
}

/** Connected drives the library does not track yet. */
export function DrivePicker({ onPick }: { onPick: (volume: Volume) => void }) {
	const { data } = useLibraryQuery({
		type: "volumes.list",
		input: { filter: "UntrackedOnly" },
	});
	const volumes = (data?.volumes ?? []).filter((volume) => volume.is_mounted);

	if (volumes.length === 0) {
		return (
			<div className="rounded-lg border border-app-line bg-app-box p-6 text-center">
				<p className="text-sm text-ink-dull">
					No untracked drives are connected. Plug one in and it appears here.
				</p>
			</div>
		);
	}

	return (
		<div className="max-h-[400px] space-y-2 overflow-y-auto pr-1">
			{volumes.map((volume) => (
				<button
					key={volume.fingerprint}
					type="button"
					onClick={() => onPick(volume)}
					className={clsx(
						"flex w-full items-center gap-3 rounded-lg border p-3 text-left",
						"transition-all hover:scale-[1.01]",
						"border-app-line bg-app-box hover:border-accent/50 hover:bg-app-hover",
					)}
				>
					<img src={HDDIcon} className="size-8" alt="" />
					<div className="min-w-0 flex-1">
						<div className="truncate text-sm font-medium text-ink">{volume.name}</div>
						<div className="truncate text-xs text-ink-faint">
							{volume.mount_point} · {fileSystemLabel(volume)}
						</div>
					</div>
					<div className="text-xs text-ink-dull">
						{volume.total_capacity ? `${(volume.total_capacity / 1e9).toFixed(0)} GB` : ""}
					</div>
				</button>
			))}
		</div>
	);
}

export function fileSystemLabel(volume: Volume): string {
	const fs = volume.file_system;
	if (!fs) return "Unknown";
	if (typeof fs === "string") return fs;
	return "Other" in fs ? fs.Other : "Unknown";
}

export function NetworkComingSoon() {
	return (
		<div className="space-y-3">
			<div className="rounded-lg border border-accent/20 bg-accent/10 p-4 text-sm text-ink">
				<strong>Coming soon</strong>
				<p className="mt-1 text-ink-dull">
					Network protocols (SMB, NFS, SFTP, WebDAV) are in development. A share
					mounted by the operating system can be added as a local folder today.
				</p>
			</div>
		</div>
	);
}
