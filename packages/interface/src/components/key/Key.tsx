import * as DropdownMenu from '@radix-ui/react-dropdown-menu';
import { useLibraryMutation } from '@sd/client';
import { Button, ContextMenu } from '@sd/ui';
import clsx from 'clsx';
import { DotsThree, Eye, Key as KeyIcon } from 'phosphor-react';
import { PropsWithChildren, useState } from 'react';
import { animated, config, useTransition } from 'react-spring';

import { DefaultProps } from '../primitive/types';
import { Tooltip } from '../tooltip/Tooltip';

export type KeyManagerProps = DefaultProps;

// TODO: Replace this with Prisma type when integrating with backend
export interface Key {
	id: string;
	name: string;
	mounted?: boolean;
	locked?: boolean;
	stats?: {
		objectCount?: number;
		containerCount?: number;
	};
	default?: boolean;
	memoryOnly?: boolean;
	// Nodes this key is mounted on
	nodes?: string[]; // will be node object
}

interface Props extends DropdownMenu.MenuContentProps {
	trigger: React.ReactNode;
	transformOrigin?: string;
	disabled?: boolean;
}

export const KeyDropdown = ({
	trigger,
	children,
	disabled,
	transformOrigin,
	className,
	...props
}: PropsWithChildren<Props>) => {
	const [open, setOpen] = useState(false);

	const transitions = useTransition(open, {
		from: {
			opacity: 0,
			transform: `scale(0.9)`,
			transformOrigin: transformOrigin || 'top'
		},
		enter: { opacity: 1, transform: 'scale(1)' },
		leave: { opacity: -0.5, transform: 'scale(0.95)' },
		config: { mass: 0.4, tension: 200, friction: 10 }
	});

	return (
		<DropdownMenu.Root open={open} onOpenChange={setOpen}>
			<DropdownMenu.Trigger>{trigger}</DropdownMenu.Trigger>
			{transitions(
				(styles, show) =>
					show && (
						<DropdownMenu.Portal forceMount>
							<DropdownMenu.Content forceMount asChild>
								<animated.div
									// most of this is copied over from the `OverlayPanel`
									className={clsx(
										'flex flex-col',
										'pl-4 pr-4 pt-2 pb-2 z-50 m-2 space-y-1',
										'select-none cursor-default rounded-lg',
										'text-left text-sm text-ink',
										'bg-app-overlay/80 backdrop-blur',
										// 'border border-app-overlay',
										'shadow-2xl shadow-black/60 ',
										className
									)}
									style={styles}
								>
									{children}
								</animated.div>
							</DropdownMenu.Content>
						</DropdownMenu.Portal>
					)
			)}
		</DropdownMenu.Root>
	);
};

export const Key: React.FC<{ data: Key; index: number }> = ({ data, index }) => {
	const mountKey = useLibraryMutation('keys.mount');
	const unmountKey = useLibraryMutation('keys.unmount');
	const deleteKey = useLibraryMutation('keys.deleteFromLibrary');
	const setDefaultKey = useLibraryMutation('keys.setDefault');

	return (
		<div
			className={clsx(
				'flex items-center justify-between px-2 py-1.5 shadow-app-shade/10 text-sm bg-app-box shadow-lg rounded-lg'
			)}
		>
			<div className="flex items-center">
				<KeyIcon
					className={clsx(
						'w-5 h-5 ml-1 mr-3',
						data.mounted ? (data.locked ? 'text-accent' : 'text-accent') : 'text-gray-400/80'
					)}
				/>
				<div className="flex flex-col ">
					<div className="flex flex-row items-center">
						<div className="font-semibold">{data.name}</div>
						{data.mounted && (
							<div className="inline ml-2 px-1 text-[8pt] font-medium text-gray-300 bg-gray-500 rounded">
								{data.nodes?.length || 0 > 0 ? `${data.nodes?.length || 0} nodes` : 'This node'}
							</div>
						)}
						{data.default && (
							<div className="inline ml-2 px-1 text-[8pt] font-medium text-gray-300 bg-gray-500 rounded">
								Default
							</div>
						)}
					</div>
					{/* <div className="text-xs text-gray-300 opacity-30">#{data.id}</div> */}
					{data.stats ? (
						<div className="flex flex-row mt-[1px] space-x-3">
							{data.stats.objectCount && (
								<div className="text-[8pt] font-medium text-ink-dull opacity-30">
									{data.stats.objectCount} Objects
								</div>
							)}
							{data.stats.containerCount && (
								<div className="text-[8pt] font-medium text-ink-dull opacity-30">
									{data.stats.containerCount} Containers
								</div>
							)}
						</div>
					) : (
						!data.mounted && (
							<div className="text-[8pt] font-medium text-ink-dull opacity-30">Key not mounted</div>
						)
					)}
				</div>
			</div>
			<div className="space-x-1">
				{data.mounted && (
					<Tooltip label="Browse files">
						<Button size="icon">
							<Eye className="w-4 h-4 text-ink-faint" />
						</Button>
					</Tooltip>
				)}
				<KeyDropdown
					trigger={
						<Button size="icon">
							<DotsThree className="w-4 h-4 text-ink-faint" />
						</Button>
					}
				>
					{data.mounted && (
						<DropdownMenu.DropdownMenuItem
							className="!cursor-default select-none text-menu-ink focus:outline-none py-0.5 active:opacity-80"
							onClick={(e) => {
								unmountKey.mutate(data.id);
							}}
						>
							Unmount
						</DropdownMenu.DropdownMenuItem>
					)}

					{!data.mounted && (
						<DropdownMenu.DropdownMenuItem
							className="!cursor-default select-none text-menu-ink focus:outline-none py-0.5 active:opacity-80"
							onClick={(e) => {
								mountKey.mutate(data.id);
							}}
						>
							Mount
						</DropdownMenu.DropdownMenuItem>
					)}

					<DropdownMenu.DropdownMenuItem
						className="!cursor-default select-none text-menu-ink focus:outline-none py-0.5 active:opacity-80"
						onClick={(e) => {
							deleteKey.mutate(data.id);
						}}
					>
						Delete from Library
					</DropdownMenu.DropdownMenuItem>

					{!data.default && (
						<DropdownMenu.DropdownMenuItem
							className="!cursor-default select-none text-menu-ink focus:outline-none py-0.5 active:opacity-80"
							onClick={(e) => {
								setDefaultKey.mutate(data.id);
							}}
						>
							Set as Default
						</DropdownMenu.DropdownMenuItem>
					)}
				</KeyDropdown>
			</div>
		</div>
	);
};

export const DummyKey = (props: { text: string }) => {
	return (
		<div className="flex items-center justify-between px-2 py-1.5 pt-2 pb-2 shadow-app-shade/10 text-sm bg-app-box shadow-lg rounded-lg">
			<div className="flex items-center">
				<KeyIcon className="w-5 h-5 ml-1 mr-3 text-gray-400/80" />
				<div className="flex flex-col ">
					<div className="flex flex-row items-center">
						<div className="font-medium">{props.text}</div>
					</div>
				</div>
			</div>
		</div>
	);
};
