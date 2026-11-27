import {
	Books,
	Check,
	DeviceMobile,
	GearSix,
	HardDrives,
	Plus
} from '@phosphor-icons/react';
import type {LibraryInfo, Layout} from '@sd/ts-client';
import {DropdownMenu, SelectPill} from '@spacedrive/primitives';
import clsx from 'clsx';
import {useCreateLibraryDialog} from '../modals/CreateLibraryModal';
import {useCreateLayoutDialog} from './CreateLayoutModal';

interface LibrarySwitcherProps {
	libraries: LibraryInfo[] | undefined;
	currentLibraryId: string | null;
	layouts: Layout[] | undefined;
	currentLayout: Layout | undefined;
	onLibrarySwitch: (libraryId: string) => void;
	onLayoutSwitch: (layoutId: string) => void;
	onManageStorage: () => void;
	onPairDevice: () => void;
	onOpenSettings: () => void;
}

export function LibrarySwitcher({
	libraries,
	currentLibraryId,
	layouts,
	currentLayout,
	onLibrarySwitch,
	onLayoutSwitch,
	onManageStorage,
	onPairDevice,
	onOpenSettings
}: LibrarySwitcherProps) {
	const currentLibrary = libraries?.find(
		(library) => library.id === currentLibraryId
	);
	const hasMultipleLayouts = (layouts?.length ?? 0) > 1;
	const createLayout = useCreateLayoutDialog;

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

				{hasMultipleLayouts && (
					<>
						<DropdownMenu.Separator className="border-sidebar-line my-1" />
						<div className="text-sidebar-ink-faint px-2 py-1 text-[10px] font-semibold uppercase tracking-wide">
							Layouts
						</div>
						{layouts?.map((layout) => (
							<DropdownMenu.Item
								key={layout.id}
								onClick={() => onLayoutSwitch(layout.id)}
								className="text-sidebar-ink-dull hover:bg-sidebar-box hover:text-sidebar-ink flex items-center gap-2 rounded-md px-2 py-1.5 text-sm"
							>
								<span
									className="size-2 rounded-full"
									style={{backgroundColor: layout.color}}
								/>
								<span className="min-w-0 flex-1 truncate">
									{layout.name}
								</span>
								{layout.id === currentLayout?.id && (
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
					onClick={() => createLayout()}
					className="text-sidebar-ink-dull hover:bg-sidebar-box hover:text-sidebar-ink flex items-center gap-2 rounded-md px-2 py-1.5 text-sm"
				>
					<Plus size={16} weight="bold" />
					New Layout
				</DropdownMenu.Item>
			</DropdownMenu.Content>
		</DropdownMenu.Root>
	);
}
