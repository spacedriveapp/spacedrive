import {
	Books,
	Check,
	DeviceMobile,
	GearSix,
	HardDrives,
	Plus
} from '@phosphor-icons/react';
import type {LibraryInfo, Space} from '@sd/ts-client';
import {DropdownMenu, SelectPill} from '@spacedrive/primitives';
import clsx from 'clsx';
import {useCreateLibraryDialog} from '../modals/CreateLibraryModal';
import {useCreateSpaceDialog} from './CreateSpaceModal';

interface LibrarySwitcherProps {
	libraries: LibraryInfo[] | undefined;
	currentLibraryId: string | null;
	spaces: Space[] | undefined;
	currentSpace: Space | undefined;
	onLibrarySwitch: (libraryId: string) => void;
	onSpaceSwitch: (spaceId: string) => void;
	onManageStorage: () => void;
	onPairDevice: () => void;
	onOpenSettings: () => void;
}

export function LibrarySwitcher({
	libraries,
	currentLibraryId,
	spaces,
	currentSpace,
	onLibrarySwitch,
	onSpaceSwitch,
	onManageStorage,
	onPairDevice,
	onOpenSettings
}: LibrarySwitcherProps) {
	const currentLibrary = libraries?.find(
		(library) => library.id === currentLibraryId
	);
	const hasMultipleSpaces = (spaces?.length ?? 0) > 1;
	const createWorkspace = useCreateSpaceDialog;

	return (
		<DropdownMenu.Root>
			<DropdownMenu.Trigger asChild>
				<SelectPill variant="sidebar" size="lg" className="shrink-0">
					<Books size={16} weight="bold" className="text-accent" />
					<span className="min-w-0 flex-1 truncate text-left">
						{currentLibrary?.name ?? 'Select Library'}
					</span>
				</SelectPill>
			</DropdownMenu.Trigger>

			<DropdownMenu.Content className="z-[100] min-w-[var(--radix-dropdown-menu-trigger-width)] p-1">
				{libraries?.map((library) => (
					<DropdownMenu.Item
						key={library.id}
						onClick={() => onLibrarySwitch(library.id)}
						className={clsx(
							'flex items-center gap-2 rounded-md px-2 py-1.5 text-sm',
							library.id === currentLibraryId
								? 'bg-sidebar-selected text-sidebar-ink'
								: 'text-sidebar-ink-dull hover:bg-sidebar-box hover:text-sidebar-ink'
						)}
					>
						<span className="min-w-0 flex-1 truncate">
							{library.name}
						</span>
						{library.id === currentLibraryId && (
							<Check size={14} weight="bold" />
						)}
					</DropdownMenu.Item>
				))}

				{hasMultipleSpaces && (
					<>
						<DropdownMenu.Separator className="border-sidebar-line my-1" />
						<div className="text-sidebar-ink-faint px-2 py-1 text-[10px] font-semibold uppercase tracking-wide">
							Workspaces
						</div>
						{spaces?.map((space) => (
							<DropdownMenu.Item
								key={space.id}
								onClick={() => onSpaceSwitch(space.id)}
								className="text-sidebar-ink-dull hover:bg-sidebar-box hover:text-sidebar-ink flex items-center gap-2 rounded-md px-2 py-1.5 text-sm"
							>
								<span
									className="size-2 rounded-full"
									style={{backgroundColor: space.color}}
								/>
								<span className="min-w-0 flex-1 truncate">
									{space.name}
								</span>
								{space.id === currentSpace?.id && (
									<Check size={14} weight="bold" />
								)}
							</DropdownMenu.Item>
						))}
					</>
				)}

				<DropdownMenu.Separator className="border-sidebar-line my-1" />
				<DropdownMenu.Item
					onClick={onManageStorage}
					className="text-sidebar-ink-dull hover:bg-sidebar-box hover:text-sidebar-ink flex items-center gap-2 rounded-md px-2 py-1.5 text-sm"
				>
					<HardDrives size={16} weight="bold" />
					Manage Storage
				</DropdownMenu.Item>
				<DropdownMenu.Item
					onClick={onPairDevice}
					className="text-sidebar-ink-dull hover:bg-sidebar-box hover:text-sidebar-ink flex items-center gap-2 rounded-md px-2 py-1.5 text-sm"
				>
					<DeviceMobile size={16} weight="bold" />
					Pair a Device
				</DropdownMenu.Item>
				<DropdownMenu.Item
					onClick={onOpenSettings}
					className="text-sidebar-ink-dull hover:bg-sidebar-box hover:text-sidebar-ink flex items-center gap-2 rounded-md px-2 py-1.5 text-sm"
				>
					<GearSix size={16} weight="bold" />
					Library Settings
				</DropdownMenu.Item>
				<DropdownMenu.Item
					onClick={() => useCreateLibraryDialog()}
					className="text-sidebar-ink-dull hover:bg-sidebar-box hover:text-sidebar-ink flex items-center gap-2 rounded-md px-2 py-1.5 text-sm"
				>
					<Plus size={16} weight="bold" />
					New Library
				</DropdownMenu.Item>
				<DropdownMenu.Item
					onClick={() => createWorkspace()}
					className="text-sidebar-ink-dull hover:bg-sidebar-box hover:text-sidebar-ink flex items-center gap-2 rounded-md px-2 py-1.5 text-sm"
				>
					<Plus size={16} weight="bold" />
					New Workspace
				</DropdownMenu.Item>
			</DropdownMenu.Content>
		</DropdownMenu.Root>
	);
}
