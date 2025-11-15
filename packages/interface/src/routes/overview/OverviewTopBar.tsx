import { useState, useEffect } from "react";
import { useNavigate } from "react-router-dom";
import {
	MagnifyingGlass,
	ArrowsClockwise,
	Plus,
	DeviceMobile,
	CaretDown,
	GearSix,
} from "@phosphor-icons/react";
import { TopBarButton, Popover, usePopover } from "@sd/ui";
import clsx from "clsx";
import { TopBarPortal } from "../../TopBar";
import { PairingModal } from "../../components/PairingModal";
import { useAddLocationDialog } from "../../components/explorer/components/AddLocationModal";
import { useSpacedriveClient } from "../../context";
import { useLibraries } from "../../hooks/useLibraries";
import { usePlatform } from "../../platform";

interface OverviewTopBarProps {
	libraryName?: string;
}

export function OverviewTopBar({ libraryName }: OverviewTopBarProps) {
	const [isPairingOpen, setIsPairingOpen] = useState(false);
	const navigate = useNavigate();
	const client = useSpacedriveClient();
	const platform = usePlatform();
	const { data: libraries } = useLibraries();
	const [currentLibraryId, setCurrentLibraryId] = useState<string | null>(
		() => client.getCurrentLibraryId(), // Initialize from client
	);
	const librarySwitcher = usePopover();

	// Listen for library changes from client and update local state
	useEffect(() => {
		const handleLibraryChange = (newLibraryId: string) => {
			setCurrentLibraryId(newLibraryId);
		};

		client.on("library-changed", handleLibraryChange);
		return () => {
			client.off("library-changed", handleLibraryChange);
		};
	}, [client]);

	// Auto-select first library on mount if none selected
	useEffect(() => {
		if (libraries && libraries.length > 0 && !currentLibraryId) {
			const firstLib = libraries[0];

			// Set library ID via platform (syncs to all windows on Tauri)
			if (platform.setCurrentLibraryId) {
				platform.setCurrentLibraryId(firstLib.id).catch((err) =>
					console.error("Failed to set library ID:", err),
				);
			} else {
				// Web fallback - just update client
				client.setCurrentLibrary(firstLib.id);
			}
		}
	}, [libraries, currentLibraryId, client, platform]);

	const handleLibrarySwitch = (libraryId: string) => {
		librarySwitcher.setOpen(false);

		// Set library ID via platform (syncs to all windows on Tauri)
		if (platform.setCurrentLibraryId) {
			platform.setCurrentLibraryId(libraryId).catch((err) =>
				console.error("Failed to set library ID:", err),
			);
		} else {
			// Web fallback - just update client
			client.setCurrentLibrary(libraryId);
		}
	};

	const currentLibrary = libraries?.find(
		(lib) => lib.id === currentLibraryId,
	);

	const handleAddLocation = () => {
		useAddLocationDialog((locationId) => {
			navigate(`/location/${locationId}`);
		});
	};

	return (
		<>
			<TopBarPortal
				left={
					<div className="flex items-center gap-3">
						<h1 className="text-xl font-bold text-ink">Overview</h1>
						<span className="text-ink-dull">•</span>
						<Popover
							popover={librarySwitcher}
							trigger={
								<button
									className={clsx(
										"flex items-center gap-1.5 rounded-lg px-3 py-1.5 text-sm font-medium",
										"bg-app-button/50 border border-app-line/50",
										"text-ink hover:bg-app-button",
										"focus:outline-none focus:ring-1 focus:ring-accent",
										"transition-colors",
										!currentLibrary && "text-ink-faint",
									)}
								>
									<span className="truncate max-w-[200px]">
										{currentLibrary?.name ||
											libraryName ||
											"Select Library"}
									</span>
									<span className="opacity-50">
										<CaretDown size={12} weight="bold" />
									</span>
								</button>
							}
							className="p-2 min-w-[200px]"
						>
							<div className="space-y-1">
								{libraries && libraries.length > 1 && (
									<>
										{libraries.map((lib) => (
											<button
												key={lib.id}
												onClick={() =>
													handleLibrarySwitch(lib.id)
												}
												className={clsx(
													"w-full px-3 py-2 text-sm rounded-md cursor-pointer text-left",
													lib.id === currentLibraryId
														? "bg-accent text-white"
														: "text-ink hover:bg-app-selected",
												)}
											>
												{lib.name}
											</button>
										))}
										<div className="border-t border-app-line my-1" />
									</>
								)}
								<button
									onClick={() =>
										librarySwitcher.setOpen(false)
									}
									className="w-full flex items-center gap-2 px-3 py-2 text-sm rounded-md hover:bg-app-selected text-ink font-medium cursor-pointer"
								>
									<Plus size={16} weight="bold" />
									<span>New Library</span>
								</button>
								<button
									onClick={() =>
										librarySwitcher.setOpen(false)
									}
									className="w-full flex items-center gap-2 px-3 py-2 text-sm rounded-md hover:bg-app-selected text-ink font-medium cursor-pointer"
								>
									<GearSix size={16} weight="bold" />
									<span>Library Settings</span>
								</button>
							</div>
						</Popover>
					</div>
				}
				right={
					<div className="flex items-center gap-2">
						<TopBarButton icon={MagnifyingGlass} title="Search" />
						<TopBarButton
							icon={DeviceMobile}
							title="Pair Device"
							onClick={() => setIsPairingOpen(true)}
						>
							Pair
						</TopBarButton>
						<TopBarButton
							icon={ArrowsClockwise}
							title="Refresh Statistics"
						>
							Refresh
						</TopBarButton>
						<TopBarButton
							icon={Plus}
							className="!bg-accent hover:!bg-accent-deep !text-white"
							onClick={handleAddLocation}
						>
							Add Location
						</TopBarButton>
					</div>
				}
			/>

			<PairingModal
				isOpen={isPairingOpen}
				onClose={() => setIsPairingOpen(false)}
			/>
		</>
	);
}
