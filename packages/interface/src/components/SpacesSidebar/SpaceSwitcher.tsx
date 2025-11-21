import clsx from 'clsx';
import { CaretDown, Plus, GearSix } from '@phosphor-icons/react';
import { DropdownMenu } from '@sd/ui';
import type { Space } from '@sd/ts-client';
import { useCreateSpaceDialog } from './CreateSpaceModal';

interface SpaceSwitcherProps {
	spaces: Space[] | undefined;
	currentSpace: Space | undefined;
	onSwitch: (spaceId: string) => void;
}

export function SpaceSwitcher({ spaces, currentSpace, onSwitch }: SpaceSwitcherProps) {
	const createSpaceDialog = useCreateSpaceDialog;

	return (
		<DropdownMenu.Root
			trigger={
				<button
					className={clsx(
						"w-full flex items-center gap-1.5 rounded-lg px-2 py-1.5 text-sm font-medium",
						"bg-sidebar-box border border-sidebar-line",
						"text-sidebar-ink hover:bg-sidebar-button",
						"focus:outline-none focus:ring-1 focus:ring-accent",
						"transition-colors",
						!currentSpace && "text-sidebar-inkFaint"
					)}
				>
					<div
						className="size-2 rounded-full shrink-0"
						style={{ backgroundColor: currentSpace?.color || '#666' }}
					/>
					<span className="truncate flex-1 text-left">
						{currentSpace?.name || 'Select Space'}
					</span>
					<CaretDown className="size-3 opacity-50" />
				</button>
			}
			className="p-1 bg-sidebar-box border border-sidebar-line rounded-lg shadow-sm overflow-hidden"
		>
			{spaces && spaces.length > 1
				? spaces.map((space) => (
						<DropdownMenu.Item
							key={space.id}
							onClick={() => onSwitch(space.id)}
							className={clsx(
								"px-2 py-1 text-sm rounded-md",
								space.id === currentSpace?.id
									? "bg-accent text-white"
									: "text-sidebar-ink hover:bg-sidebar-selected"
							)}
						>
							<div className="flex items-center gap-2">
								<div
									className="size-2 rounded-full"
									style={{ backgroundColor: space.color }}
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
				icon={Plus}
				onClick={() => createSpaceDialog()}
				className="px-2 py-1 text-sm rounded-md hover:bg-sidebar-selected text-sidebar-ink font-medium"
			>
				New Space
			</DropdownMenu.Item>
			<DropdownMenu.Item
				icon={GearSix}
				className="px-2 py-1 text-sm rounded-md hover:bg-sidebar-selected text-sidebar-ink font-medium"
			>
				Space Settings
			</DropdownMenu.Item>
		</DropdownMenu.Root>
	);
}
