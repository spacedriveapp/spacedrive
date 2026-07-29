import {GearSix, Plus} from '@phosphor-icons/react';
import type {Space} from '@sd/ts-client';
import {DropdownMenu, SelectPill} from '@spacedrive/primitives';
import clsx from 'clsx';
import {useState} from 'react';
import {useCreateSpaceDialog} from './CreateSpaceModal';
import {useSpaceSettingsDialog} from './SpaceSettingsModal';

interface SpaceSwitcherProps {
	spaces: Space[] | undefined;
	currentSpace: Space | undefined;
	onSwitch: (spaceId: string) => void;
}

export function SpaceSwitcher({
	spaces,
	currentSpace,
	onSwitch
}: SpaceSwitcherProps) {
	const [menuOpen, setMenuOpen] = useState(false);

	const openDialogAfterMenuCloses = (openDialog: () => void) => {
		setMenuOpen(false);
		// Let the dropdown fully unmount before opening a modal overlay.
		requestAnimationFrame(() => {
			openDialog();
		});
	};

	return (
		<DropdownMenu.Root open={menuOpen} onOpenChange={setMenuOpen}>
			<DropdownMenu.Trigger asChild>
				<SelectPill variant="sidebar" size="lg" className="shrink-0">
					<div
						className="size-2 rounded-full"
						style={{backgroundColor: currentSpace?.color || '#666'}}
					/>
					<span className="flex-1 truncate text-left">
						{currentSpace?.name || 'Select Space'}
					</span>
				</SelectPill>
			</DropdownMenu.Trigger>
			<DropdownMenu.Content
				sideOffset={4}
				className="border-sidebar-line !bg-sidebar-box !backdrop-blur-none z-[70] min-w-[var(--radix-dropdown-menu-trigger-width)] p-1 shadow-xl"
			>
				{spaces && spaces.length > 1
					? spaces.map((space) => (
							<DropdownMenu.Item
								key={space.id}
								onSelect={(event) => {
									event.preventDefault();
									onSwitch(space.id);
								}}
								className={clsx(
									'rounded-md px-2 py-1 text-sm',
									space.id === currentSpace?.id
										? 'bg-accent text-white'
										: 'text-sidebar-ink hover:bg-sidebar-selected'
								)}
							>
								<div className="flex items-center gap-2">
									<div
										className="size-2 rounded-full"
										style={{backgroundColor: space.color}}
									/>
									<span>{space.name}</span>
								</div>
							</DropdownMenu.Item>
						))
					: null}
				{spaces && spaces.length > 1 && (
					<DropdownMenu.Separator className="border-sidebar-line my-1" />
				)}
				<DropdownMenu.Item
					onSelect={(event) => {
						event.preventDefault();
						openDialogAfterMenuCloses(useCreateSpaceDialog);
					}}
					className="hover:bg-sidebar-selected text-sidebar-ink rounded-md px-2 py-1 text-sm font-medium"
				>
					<Plus className="mr-2 size-4" weight="bold" />
					New Space
				</DropdownMenu.Item>
				<DropdownMenu.Item
					disabled={!currentSpace}
					onSelect={(event) => {
						event.preventDefault();
						if (!currentSpace) return;
						openDialogAfterMenuCloses(() =>
							useSpaceSettingsDialog(currentSpace),
						);
					}}
					className="hover:bg-sidebar-selected text-sidebar-ink rounded-md px-2 py-1 text-sm font-medium"
				>
					<GearSix className="mr-2 size-4" weight="bold" />
					Space Settings
				</DropdownMenu.Item>
			</DropdownMenu.Content>
		</DropdownMenu.Root>
	);
}