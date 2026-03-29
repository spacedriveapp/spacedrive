import {CaretDown, GearSix, Plus} from '@phosphor-icons/react';
import type {Space} from '@sd/ts-client';
import {DropdownMenu, SelectPill} from '@spaceui/primitives';
import clsx from 'clsx';
import {useCreateSpaceDialog} from './CreateSpaceModal';

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
	const createSpaceDialog = useCreateSpaceDialog;

	return (
		<DropdownMenu.Root>
			<DropdownMenu.Trigger asChild>
				<SelectPill variant="sidebar" size="lg">
					<div
						className="size-2 rounded-full"
						style={{backgroundColor: currentSpace?.color || '#666'}}
					/>
					<span className="flex-1 truncate text-left">
						{currentSpace?.name || 'Select Space'}
					</span>
				</SelectPill>
			</DropdownMenu.Trigger>
			<DropdownMenu.Content className="min-w-[var(--radix-dropdown-menu-trigger-width)] p-1">
				{spaces && spaces.length > 1
					? spaces.map((space) => (
							<DropdownMenu.Item
								key={space.id}
								onClick={() => onSwitch(space.id)}
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
					onClick={() => createSpaceDialog()}
					className="hover:bg-sidebar-selected text-sidebar-ink rounded-md px-2 py-1 text-sm font-medium"
				>
					<Plus className="mr-2 size-4" weight="bold" />
					New Space
				</DropdownMenu.Item>
				<DropdownMenu.Item className="hover:bg-sidebar-selected text-sidebar-ink rounded-md px-2 py-1 text-sm font-medium">
					<GearSix className="mr-2 size-4" weight="bold" />
					Space Settings
				</DropdownMenu.Item>
			</DropdownMenu.Content>
		</DropdownMenu.Root>
	);
}
