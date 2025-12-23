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
import { getDeviceIcon, useLibraryMutation } from "@sd/ts-client";
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
	const [isEditing, setIsEditing] = useState(false);
	const [editValue, setEditValue] = useState("");
	const [editingAsUri, setEditingAsUri] = useState(false);
	const { navigateToView } = useExplorer();
	const uri = sdPathToUri(path);
	const currentDir = getCurrentDirectoryName(path);
	const segments = parsePathSegments(path);

	// Get device icon and device info based on the device_slug
	const deviceInfo = (() => {
		if ("Physical" in path) {
			const deviceSlug = path.Physical.device_slug;
			// Find device by slug
			const device = Array.from(devices.values()).find(
				(d) => d.slug === deviceSlug,
			);
			return {
				icon: device ? getDeviceIcon(device) : LaptopIcon,
				device,
			};
		}
		// For Cloud paths, we don't have a device
		return { icon: LaptopIcon, device: undefined };
	})();

	const handleDeviceClick = () => {
		if (deviceInfo.device) {
			navigateToView("device", deviceInfo.device.id);
		}
	};

	const enterEditMode = (initialValue: string, asUri: boolean) => {
		setIsEditing(true);
		setEditValue(initialValue);
		setEditingAsUri(asUri);
	};

	const exitEditMode = () => {
		setIsEditing(false);
		setEditValue("");
		setEditingAsUri(false);
	};

	const handleContainerClick = (e: React.MouseEvent) => {
		// Only enter edit mode if clicking the container itself, not buttons/segments
		if (e.target === e.currentTarget || (e.target as HTMLElement).tagName === "INPUT") {
			const isUriMode = showUri;
			const valueToEdit = isUriMode ? uri : ("Physical" in path ? path.Physical.path : uri);
			enterEditMode(valueToEdit, isUriMode);
		}
	};

	const handleEditKeyDown = (e: React.KeyboardEvent<HTMLInputElement>) => {
		if (e.key === "Enter") {
			e.preventDefault();
			submitEdit();
		} else if (e.key === "Escape") {
			e.preventDefault();
			exitEditMode();
		}
	};

	const submitEdit = () => {
		const trimmed = editValue.trim();
		if (!trimmed) {
			exitEditMode();
			return;
		}

		try {
			if (editingAsUri) {
				// Try to parse as SdPath JSON
				const parsed = JSON.parse(trimmed) as SdPath;
				onNavigate(parsed);
			} else {
				// Parse as file path string
				if ("Physical" in path) {
					const newPath: SdPath = {
						Physical: {
							device_slug: path.Physical.device_slug,
							path: trimmed.startsWith("/") ? trimmed : `/${trimmed}`,
						},
					};
					onNavigate(newPath);
				}
			}
		} catch (error) {
			console.error("Failed to parse path:", error);
		}
		
		exitEditMode();
	};

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

	// Calculate widths for different states
	const collapsedWidth = currentDir.length * 8.5 + 70;
	const breadcrumbsWidth = Math.min(
		segments.reduce((sum, seg) => sum + seg.name.length * 6.5, 0) +
			(segments.length - 1) * 16 + // separators
			70, // base padding + icon
		600,
	);
	const uriWidth = Math.min(uri.length * 7 + 70, 600);
	const editWidth = Math.max(200, Math.min(editValue.length * 7 + 70, 600));

	const currentWidth = isEditing
		? editWidth
		: !isExpanded
			? collapsedWidth
			: showUri
				? uriWidth
				: breadcrumbsWidth;

	return (
		<div className="flex items-center gap-2">
			<motion.div
				animate={{ width: currentWidth }}
				transition={{ duration: 0.2, ease: [0.25, 1, 0.5, 1] }}
				onMouseEnter={() => !isEditing && setIsExpanded(true)}
				onMouseLeave={() => !isEditing && setIsExpanded(false)}
				onClick={handleContainerClick}
				className={clsx(
					"flex items-center gap-1.5 h-8 px-3 rounded-full",
					"backdrop-blur-xl border border-sidebar-line/30",
					"bg-sidebar-box/20 transition-colors",
					"focus-within:bg-sidebar-box/30 focus-within:border-sidebar-line/40",
					!isEditing && "cursor-text",
				)}
			>
				<button
					onClick={(e) => {
						e.stopPropagation();
						handleDeviceClick();
					}}
					disabled={!deviceInfo.device}
					title={
						deviceInfo.device
							? `Go to ${deviceInfo.device.name}`
							: "Device"
					}
					className={clsx(
						"size-5 flex-shrink-0 transition-opacity",
						deviceInfo.device
							? "opacity-60 hover:opacity-100 cursor-pointer"
							: "opacity-60 cursor-default",
					)}
				>
					<img
						src={deviceInfo.icon}
						alt="Device"
						className="size-full"
					/>
				</button>

				{isEditing ? (
					<input
						type="text"
						value={editValue}
						onChange={(e) => setEditValue(e.target.value)}
						onKeyDown={handleEditKeyDown}
						onBlur={exitEditMode}
						autoFocus
						className={clsx(
							"bg-transparent border-0 outline-none ring-0 flex-1 min-w-0",
							"text-xs font-medium text-sidebar-ink",
							"placeholder:text-sidebar-inkFaint",
							"focus:ring-0 focus:outline-none",
							editingAsUri && "font-mono",
						)}
						placeholder={editingAsUri ? "Enter SdPath JSON..." : "Enter path..."}
					/>
				) : showUri ? (
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
										onClick={(e) => {
											e.stopPropagation();
											!isLast && onNavigate(segment.path);
										}}
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
									{!isLast && (
										<button
											onClick={(e) => {
												e.stopPropagation();
												const valueToEdit = "Physical" in path ? path.Physical.path : uri;
												enterEditMode(valueToEdit, false);
											}}
											className="opacity-50 hover:opacity-100 transition-opacity cursor-text"
										>
											<CaretRight size={12} />
										</button>
									)}
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
