import { useState, useEffect } from "react";
import { useNavigate } from "react-router-dom";
import {
	MagnifyingGlass,
	ArrowsClockwise,
	Plus,
	DeviceMobile,
	CaretDown,
	GearSix,
	CloudArrowUp,
} from "@phosphor-icons/react";
import { TopBarButton, Popover, usePopover } from "@sd/ui";
import clsx from "clsx";
import { TopBarPortal } from "../../TopBar";
import { PairingModal } from "../../components/PairingModal";
import { useAddStorageDialog } from "../../components/Explorer/components/AddStorageModal";
import { useSyncSetupDialog } from "../../components/SyncSetupModal";
import { useCreateLibraryDialog } from "../../components/CreateLibraryModal";
import { useSpacedriveClient } from "../../context";
import { useLibraries } from "../../hooks/useLibraries";
import { usePlatform } from "../../platform";
import { useLibraryMutation } from "@sd/ts-client";

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
				platform
					.setCurrentLibraryId(firstLib.id)
					.catch((err) =>
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
			platform
				.setCurrentLibraryId(libraryId)
				.catch((err) =>
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

	const handleAddStorage = () => {
		useAddStorageDialog((id) => {
			navigate(`/location/${id}`);
		});
	};

	const handleSyncSetup = () => {
		useSyncSetupDialog();
	};

	// Mutation for refreshing volume statistics
	const volumeRefreshMutation = useLibraryMutation("volumes.refresh");
	const [isRefreshing, setIsRefreshing] = useState(false);

	const handleRefresh = async () => {
		setIsRefreshing(true);
		try {
			const result = (await volumeRefreshMutation.mutateAsync({
				force: false,
			} as any)) as any;
			console.log(
				`Volume refresh complete: ${result.volumes_refreshed} refreshed, ${result.volumes_failed} failed`,
			);
		} catch (error) {
			console.error("Failed to refresh volumes:", error);
		} finally {
			setIsRefreshing(false);
		}
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
										"flex items-center gap-2 h-8 px-3 rounded-full text-xs font-medium",
										"backdrop-blur-xl transition-all",
										"border border-sidebar-line/30",
										"bg-sidebar-box/20 text-sidebar-inkDull hover:bg-sidebar-box/30 hover:text-sidebar-ink",
										"active:scale-95",
										!currentLibrary && "text-ink-faint",
									)}
								>
									<span className="truncate max-w-[200px]">
										{currentLibrary?.name ||
											libraryName ||
											"Select Library"}
									</span>
									<CaretDown size={12} weight="bold" />
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
									onClick={() => {
										librarySwitcher.setOpen(false);
										useCreateLibraryDialog();
									}}
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
							icon={CloudArrowUp}
							title="Setup Sync"
							onClick={handleSyncSetup}
						>
							Setup Sync
						</TopBarButton>
						<TopBarButton
							icon={ArrowsClockwise}
							title="Refresh Statistics"
							onClick={handleRefresh}
							disabled={isRefreshing}
							className={clsx(isRefreshing && "animate-spin")}
						>
							Refresh
						</TopBarButton>
						<TopBarButton
							icon={Plus}
							className="!bg-accent hover:!bg-accent-deep !text-white"
							onClick={handleAddStorage}
						>
							Add Storage
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
