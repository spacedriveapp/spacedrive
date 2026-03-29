import {
	CaretDown,
	Clock,
	GearSix,
	Heart,
	House,
	Network,
	Planet,
	Plus,
	Tag
} from '@phosphor-icons/react';
import {DropdownMenu} from '@spaceui/primitives';
import clsx from 'clsx';
import {useEffect, useState} from 'react';
import {useLocation, useNavigate} from 'react-router-dom';
import {JobManagerPopover} from '../../components/JobManager';
import {SyncMonitorPopover} from '../../components/SyncMonitor';
import {usePlatform} from '../../contexts/PlatformContext';
import {useSpacedriveClient} from '../../contexts/SpacedriveContext';
import {useLibraries} from '../../hooks/useLibraries';
import {LocationsSection} from './components/LocationsSection';
import {Section} from './components/Section';
import {SidebarItem} from './components/SidebarItem';

export function Sidebar() {
	const client = useSpacedriveClient();
	const platform = usePlatform();
	const {data: libraries} = useLibraries();
	const navigate = useNavigate();
	const location = useLocation();
	const [currentLibraryId, setCurrentLibraryId] = useState<string | null>(
		() => client.getCurrentLibraryId()
	);

	const isActive = (path: string) => location.pathname === path;

	// Listen for library changes from client and update local state
	useEffect(() => {
		const handleLibraryChange = (newLibraryId: string) => {
			setCurrentLibraryId(newLibraryId);
		};

		client.on('library-changed', handleLibraryChange);
		return () => {
			client.off('library-changed', handleLibraryChange);
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
						console.error('Failed to set library ID:', err)
					);
			} else {
				// Web fallback - just update client
				client.setCurrentLibrary(firstLib.id);
			}
		}
	}, [libraries, currentLibraryId, client, platform]);

	const handleLibrarySwitch = (libraryId: string) => {
		// Set library ID via platform (syncs to all windows on Tauri)
		if (platform.setCurrentLibraryId) {
			platform
				.setCurrentLibraryId(libraryId)
				.catch((err) =>
					console.error('Failed to set library ID:', err)
				);
		} else {
			// Web fallback - just update client
			client.setCurrentLibrary(libraryId);
		}
	};

	const currentLibrary = libraries?.find(
		(lib) => lib.id === currentLibraryId
	);

	return (
		<div className="bg-app flex h-full w-[220px] min-w-[176px] max-w-[300px] flex-col p-2">
			<div
				className={clsx(
					'flex h-full flex-col overflow-hidden rounded-2xl',
					'bg-sidebar/65'
				)}
			>
				<nav className="relative z-[51] flex h-full flex-col gap-2.5 p-2.5 pb-2 pt-[52px]">
					<DropdownMenu.Root>
						<DropdownMenu.Trigger asChild>
							<button
								className={clsx(
									'flex w-full items-center gap-1.5 rounded-lg px-2 py-1.5 text-sm font-medium',
									'bg-sidebar-box border-sidebar-line border',
									'text-sidebar-ink hover:bg-sidebar-button',
									'focus:ring-accent focus:outline-none focus:ring-1',
									'transition-colors',
									!currentLibrary && 'text-sidebar-inkFaint'
								)}
							>
								<span className="flex-1 truncate text-left">
									{currentLibrary?.name || 'Select Library'}
								</span>
								<CaretDown className="size-3 opacity-50" />
							</button>
						</DropdownMenu.Trigger>
						<DropdownMenu.Content className="min-w-[var(--radix-dropdown-menu-trigger-width)]">
							{libraries && libraries.length > 1
								? libraries.map((lib) => (
										<DropdownMenu.Item
											key={lib.id}
											onClick={() =>
												handleLibrarySwitch(lib.id)
											}
											className={clsx(
												'rounded-md px-2 py-1 text-sm',
												lib.id === currentLibraryId
													? 'bg-accent text-white'
													: 'text-sidebar-ink hover:bg-sidebar-selected'
											)}
										>
											{lib.name}
										</DropdownMenu.Item>
									))
								: null}
							{libraries && libraries.length > 1 && (
								<DropdownMenu.Separator className="border-sidebar-line my-1" />
							)}
							<DropdownMenu.Item className="hover:bg-sidebar-selected text-sidebar-ink rounded-md px-2 py-1 text-sm font-medium">
								<Plus className="mr-2 size-4" weight="bold" />
								New Library
							</DropdownMenu.Item>
							<DropdownMenu.Item className="hover:bg-sidebar-selected text-sidebar-ink rounded-md px-2 py-1 text-sm font-medium">
								<GearSix
									className="mr-2 size-4"
									weight="bold"
								/>
								Library Settings
							</DropdownMenu.Item>
						</DropdownMenu.Content>
					</DropdownMenu.Root>

					<div className="no-scrollbar mask-fade-out flex grow flex-col space-y-5 overflow-x-hidden overflow-y-scroll pb-10">
						<div className="space-y-0.5">
							<SidebarItem
								icon={Planet}
								label="Overview"
								active={isActive('/')}
								weight={isActive('/') ? 'fill' : 'bold'}
								onClick={() => navigate('/')}
							/>
							<SidebarItem
								icon={Clock}
								label="Recents"
								active={isActive('/recents')}
								onClick={() => navigate('/recents')}
							/>
							<SidebarItem
								icon={Heart}
								label="Favorites"
								active={isActive('/favorites')}
								onClick={() => navigate('/favorites')}
							/>
						</div>

						<LocationsSection />

						<Section title="Tags">
							<SidebarItem
								icon={Tag}
								label="Work"
								color="#3B82F6"
							/>
							<SidebarItem
								icon={Tag}
								label="Personal"
								color="#10B981"
							/>
							<SidebarItem
								icon={Tag}
								label="Archive"
								color="#F59E0B"
							/>
						</Section>

						<Section title="Cloud">
							<SidebarItem icon={Network} label="Sync" />
						</Section>
					</div>

					<div className="space-y-0.5">
						<SidebarItem icon={GearSix} label="Settings" />
					</div>

					<div className="mt-2">
						<JobManagerPopover />
					</div>
				</nav>
			</div>
		</div>
	);
}
