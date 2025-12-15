import { useState, useEffect } from "react";
import { motion } from "framer-motion";
import clsx from "clsx";
import {
	CaretRight,
	CircleDashedIcon,
	CircleIcon,
	Eye,
	Folder,
	RadioButtonIcon,
} from "@phosphor-icons/react";
import type { SdPath, LibraryDeviceInfo } from "@sd/ts-client";
import { getDeviceIconBySlug, useLibraryMutation } from "@sd/ts-client";
import { sdPathToUri } from "../utils";
import LaptopIcon from "@sd/assets/icons/Laptop.png";
import { useNormalizedQuery } from "@sd/ts-client";
import {
	TopBarButton,
	Popover,
	usePopover,
	PopoverContainer,
	PopoverSection,
	PopoverDivider,
	Button,
} from "@sd/ui";
import { useSelection } from "../SelectionContext";
import { useAddStorageDialog } from "./AddStorageModal";
import { useExplorer } from "../context";

interface PathBarProps {
	path: SdPath;
	devices: Map<string, LibraryDeviceInfo>;
	onNavigate: (path: SdPath) => void;
}

interface PathSegment {
	name: string;
	path: SdPath;
}

function getCurrentDirectoryName(sdPath: SdPath): string {
	if ("Physical" in sdPath) {
		const parts = sdPath.Physical.path.split("/").filter(Boolean);
		return parts[parts.length - 1] || "/";
	}

	if ("Cloud" in sdPath) {
		const parts = sdPath.Cloud.path.split("/").filter(Boolean);
		return parts[parts.length - 1] || sdPath.Cloud.identifier;
	}

	if ("Content" in sdPath) {
		return "Content";
	}

	return "";
}

function parsePathSegments(sdPath: SdPath): PathSegment[] {
	if ("Physical" in sdPath) {
		const { device_slug, path } = sdPath.Physical;
		const parts = path.split("/").filter(Boolean);

		return [
			{
				name: `/`,
				path: {
					Physical: {
						device_slug,
						path: "/",
					},
				},
			},
			...parts.map((part, index) => ({
				name: part,
				path: {
					Physical: {
						device_slug,
						path: "/" + parts.slice(0, index + 1).join("/"),
					},
				},
			})),
		];
	}

	if ("Cloud" in sdPath) {
		const { service, identifier, path } = sdPath.Cloud;
		const parts = path.split("/").filter(Boolean);

		return [
			{
				name: identifier,
				path: {
					Cloud: {
						service,
						identifier,
						path: "",
					},
				},
			},
			...parts.map((part, index) => ({
				name: part,
				path: {
					Cloud: {
						service,
						identifier,
						path: parts.slice(0, index + 1).join("/"),
					},
				},
			})),
		];
	}

	return [];
}

function IndexIndicator({ path }: { path: SdPath }) {
	const popover = usePopover();
	const enableIndexing = useLibraryMutation("locations.enable_indexing");
	const { clearSelection } = useSelection();
	const { setInspectorVisible } = useExplorer();

	// Fetch all locations
	const { data: locationsData } = useNormalizedQuery({
		wireMethod: "query:locations.list",
		input: null,
		resourceType: "location",
	});

	const locations = (locationsData as any)?.locations ?? [];

	// Find location that contains this path
	const matchingLocation = (() => {
		if ("Physical" in path) {
			const pathStr = path.Physical.path;
			// Find location with longest matching prefix
			return locations
				.filter((loc) => {
					if (!loc.sd_path || !("Physical" in loc.sd_path))
						return false;
					const locPath = loc.sd_path.Physical.path;
					return pathStr.startsWith(locPath);
				})
				.sort((a, b) => {
					const aPath =
						"Physical" in a.sd_path!
							? a.sd_path!.Physical.path
							: "";
					const bPath =
						"Physical" in b.sd_path!
							? b.sd_path!.Physical.path
							: "";
					return bPath.length - aPath.length;
				})[0];
		}
		return undefined;
	})();

	const isIndexed =
		matchingLocation?.index_mode !== undefined &&
		matchingLocation.index_mode !== "none";

	return (
		<Popover
			popover={popover}
			trigger={
				<TopBarButton
					icon={isIndexed ? CircleIcon : CircleDashedIcon}
					active={isIndexed}
					className={isIndexed ? "!text-accent" : undefined}
					title={isIndexed ? "Location is indexed" : "Not indexed"}
				/>
			}
		>
			<PopoverContainer>
				{matchingLocation ? (
					<>
						<PopoverSection>
							<div className="px-2 py-1.5">
								<div className="text-xs font-semibold text-ink">
									{matchingLocation.name}
								</div>
								<div className="text-xs text-ink-dull mt-0.5">
									{isIndexed
										? `Indexed (${matchingLocation.index_mode})`
										: "Not indexed"}
								</div>
							</div>
						</PopoverSection>

						<PopoverDivider />

						<PopoverSection>
							{!isIndexed && (
								<button
									onClick={async () => {
										await enableIndexing.mutateAsync({
											id: matchingLocation.id,
											index_mode: "deep",
										});
										popover.setOpen(false);
									}}
									className="flex items-center gap-2 px-2 py-1.5 rounded-md text-xs font-medium text-ink hover:bg-app-hover transition-colors"
								>
									<Eye size={16} />
									Enable Indexing
								</button>
							)}
							<button
								onClick={() => {
									clearSelection();
									setInspectorVisible(true);
									popover.setOpen(false);
								}}
								className="flex items-center gap-2 px-2 py-1.5 rounded-md text-xs font-medium text-ink hover:bg-app-hover transition-colors"
							>
								<Folder size={16} />
								Open Location Inspector
							</button>
						</PopoverSection>
					</>
				) : (
					<PopoverSection>
						<div className="px-2 py-1.5">
							<div className="text-xs text-ink-dull mb-2">
								Path is outside any location
							</div>
							<Button
								size="sm"
								variant="accent"
								onClick={() => {
									const initialPath =
										"Physical" in path
											? path.Physical.path
											: undefined;
									useAddStorageDialog(undefined, initialPath);
									popover.setOpen(false);
								}}
							>
								Add Location
							</Button>
						</div>
					</PopoverSection>
				)}
			</PopoverContainer>
		</Popover>
	);
}

export function PathBar({ path, devices, onNavigate }: PathBarProps) {
	const [isExpanded, setIsExpanded] = useState(false);
	const [isShiftHeld, setIsShiftHeld] = useState(false);
	const uri = sdPathToUri(path);
	const currentDir = getCurrentDirectoryName(path);
	const segments = parsePathSegments(path);

	// Get device icon based on the device_slug
	const deviceIcon = (() => {
		if ("Physical" in path) {
			return getDeviceIconBySlug(path.Physical.device_slug, devices);
		}
		// For Cloud paths, we don't have a device icon
		return LaptopIcon;
	})();

	useEffect(() => {
		const handleKeyDown = (e: KeyboardEvent) => {
			if (e.key === "Shift") setIsShiftHeld(true);
		};
		const handleKeyUp = (e: KeyboardEvent) => {
			if (e.key === "Shift") setIsShiftHeld(false);
		};

		window.addEventListener("keydown", handleKeyDown);
		window.addEventListener("keyup", handleKeyUp);

		return () => {
			window.removeEventListener("keydown", handleKeyDown);
			window.removeEventListener("keyup", handleKeyUp);
		};
	}, []);

	const showUri = isExpanded && isShiftHeld;

	// Calculate widths for three states
	const collapsedWidth = currentDir.length * 8.5 + 70;
	const breadcrumbsWidth = Math.min(
		segments.reduce((sum, seg) => sum + seg.name.length * 6.5, 0) +
			(segments.length - 1) * 16 + // separators
			70, // base padding + icon
		600,
	);
	const uriWidth = Math.min(uri.length * 7 + 70, 600);

	const currentWidth = !isExpanded
		? collapsedWidth
		: showUri
			? uriWidth
			: breadcrumbsWidth;

	return (
		<div className="flex items-center gap-2">
			<motion.div
				animate={{ width: currentWidth }}
				transition={{ duration: 0.2, ease: [0.25, 1, 0.5, 1] }}
				onMouseEnter={() => setIsExpanded(true)}
				onMouseLeave={() => setIsExpanded(false)}
				className={clsx(
					"flex items-center gap-1.5 h-8 px-3 rounded-full",
					"backdrop-blur-xl border border-sidebar-line/30",
					"bg-sidebar-box/20 transition-colors",
					"focus-within:bg-sidebar-box/30 focus-within:border-sidebar-line/40",
				)}
			>
				<img
					src={deviceIcon}
					alt="Device"
					className="size-5 opacity-60 flex-shrink-0"
				/>

				{showUri ? (
					<input
						type="text"
						value={uri}
						readOnly
						className={clsx(
							"bg-transparent border-0 outline-none ring-0 flex-1 min-w-0",
							"text-xs font-medium text-sidebar-ink",
							"placeholder:text-sidebar-inkFaint",
							"select-all cursor-text",
							"focus:ring-0 focus:outline-none",
						)}
						placeholder="No path selected"
					/>
				) : isExpanded ? (
					<div className="flex items-center gap-1 flex-1 min-w-0 overflow-hidden">
						{segments.map((segment, index) => {
							const isLast = index === segments.length - 1;
							return (
								<div
									key={index}
									className="flex items-center gap-1 flex-shrink-0"
								>
									<button
										onClick={() =>
											!isLast && onNavigate(segment.path)
										}
										disabled={isLast}
										className={clsx(
											"text-xs font-medium transition-colors whitespace-nowrap",
											isLast
												? "text-sidebar-ink cursor-default"
												: "text-sidebar-inkDull hover:text-sidebar-ink cursor-pointer",
										)}
									>
										{segment.name}
									</button>
									{!isLast && <CaretRight size={12} />}
								</div>
							);
						})}
					</div>
				) : (
					<input
						type="text"
						value={currentDir}
						readOnly
						className={clsx(
							"bg-transparent border-0 outline-none ring-0 flex-1 min-w-0",
							"text-xs font-medium text-sidebar-ink",
							"placeholder:text-sidebar-inkFaint",
							"select-all cursor-text",
							"focus:ring-0 focus:outline-none",
						)}
						placeholder="No path selected"
					/>
				)}
			</motion.div>
			<IndexIndicator path={path} />
		</div>
	);
}
