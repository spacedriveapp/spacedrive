import * as DropdownMenu from '@radix-ui/react-dropdown-menu';
import clsx from 'clsx';
import { DotsThree, Eye, Key as KeyIcon } from 'phosphor-react';
import { PropsWithChildren, useState } from 'react';
import { animated, useTransition } from 'react-spring';
import { useLibraryMutation } from '@sd/client';
import { Button, Tooltip } from '@sd/ui';

// TODO: Replace this with Prisma type when integrating with backend
export interface Key {
	id: string;
	name: string;
	queue: Set<string>;
	mounted?: boolean;
	locked?: boolean;
	stats?: {
		objectCount?: number;
		containerCount?: number;
	};
	default?: boolean;
	memoryOnly?: boolean;
	automount?: boolean;
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
	transformOrigin,
	className
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
										'z-50 m-2 space-y-1 px-4 py-2',
										'cursor-default select-none rounded-lg',
										'text-ink text-left text-sm',
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

export const Key = ({ data }: { data: Key }) => {
	const mountKey = useLibraryMutation('keys.mount');
	const unmountKey = useLibraryMutation('keys.unmount');
	const deleteKey = useLibraryMutation('keys.deleteFromLibrary');
	const setDefaultKey = useLibraryMutation('keys.setDefault');
	const changeAutomountStatus = useLibraryMutation('keys.updateAutomountStatus');
	const syncToLibrary = useLibraryMutation('keys.syncKeyToLibrary');

	if (data.mounted && data.queue.has(data.id)) {
		data.queue.delete(data.id);
	}

	return (
		<div
			className={clsx(
				'shadow-app-shade/10 bg-app-box flex items-center justify-between rounded-lg px-2 py-1.5 text-sm shadow-lg'
			)}
		>
			<div className="flex items-center">
				<KeyIcon
					className={clsx(
						'ml-1 mr-3 h-5 w-5',
						data.mounted ? (data.locked ? 'text-accent' : 'text-accent') : 'text-gray-400/80'
					)}
				/>
				<div className="flex flex-col ">
					<div className="flex flex-row items-center">
						<div className="font-semibold">{data.name}</div>
						{data.mounted && (
							<div className="ml-2 inline rounded bg-gray-500 px-1 text-[8pt] font-medium text-gray-300">
								{data.nodes?.length || 0 > 0 ? `${data.nodes?.length || 0} nodes` : 'This node'}
							</div>
						)}
						{data.default && (
							<div className="ml-2 inline rounded bg-gray-500 px-1 text-[8pt] font-medium text-gray-300">
								Default
							</div>
						)}
					</div>
					{/* <div className="text-xs text-gray-300 opacity-30">#{data.id}</div> */}
					{data.stats ? (
						<div className="mt-[1px] flex flex-row space-x-3">
							{data.stats.objectCount && (
								<div className="text-ink-dull text-[8pt] font-medium opacity-30">
									{data.stats.objectCount} Objects
								</div>
							)}
							{data.stats.containerCount && (
								<div className="text-ink-dull text-[8pt] font-medium opacity-30">
									{data.stats.containerCount} Containers
								</div>
							)}
						</div>
					) : (
						!data.mounted && (
							<div className="text-ink-dull text-[8pt] font-medium opacity-30">
								{data.queue.has(data.id) ? 'Key mounting...' : 'Key not mounted'}
							</div>
						)
					)}
				</div>
			</div>
			<div className="space-x-1">
				{data.mounted && (
					<Tooltip label="Browse files">
						<Button size="icon">
							<Eye className="text-ink-faint h-4 w-4" />
						</Button>
					</Tooltip>
				)}
				<KeyDropdown
					trigger={
						<Button size="icon">
							<DotsThree className="text-ink-faint h-4 w-4" />
						</Button>
					}
				>
					<KeyDropdownItem
						onClick={() => {
							unmountKey.mutate(data.id);
						}}
						hidden={!data.mounted}
						value="Unmount"
					/>
					<KeyDropdownItem
						onClick={() => {
							syncToLibrary.mutate(data.id);
						}}
						hidden={!data.memoryOnly}
						value="Sync to library"
					/>
					<KeyDropdownItem
						onClick={() => {
							data.queue.add(data.id);
							mountKey.mutate(data.id);
						}}
						hidden={data.mounted || data.queue.has(data.id)}
						value="Mount"
					/>
					<KeyDropdownItem
						onClick={() => {
							deleteKey.mutate(data.id);
						}}
						value="Delete from Library"
					/>
					<KeyDropdownItem
						onClick={() => {
							setDefaultKey.mutate(data.id);
						}}
						hidden={data.default}
						value="Set as Default"
					/>
					<KeyDropdownItem
						onClick={() => {
							changeAutomountStatus.mutate({ uuid: data.id, status: false });
						}}
						hidden={!data.automount || data.memoryOnly}
						value="Disable Automount"
					/>
					<KeyDropdownItem
						onClick={() => {
							changeAutomountStatus.mutate({ uuid: data.id, status: true });
						}}
						hidden={data.automount || data.memoryOnly}
						value="Enable Automount"
					/>
				</KeyDropdown>
			</div>
		</div>
	);
};

export const KeyDropdownItem = (props: {
	value: string;
	hidden?: boolean | undefined;
	onClick: () => void;
}) => {
	return (
		<DropdownMenu.DropdownMenuItem
			className="text-menu-ink !cursor-default select-none py-0.5 focus:outline-none active:opacity-80"
			onClick={props.onClick}
			hidden={props.hidden}
		>
			{props.value}
		</DropdownMenu.DropdownMenuItem>
	);
};

export const DummyKey = (props: { text: string }) => {
	return (
		<div className="shadow-app-shade/10 bg-app-box flex items-center justify-between rounded-lg p-2 py-1.5 text-sm shadow-lg">
			<div className="flex items-center">
				<KeyIcon className="ml-1 mr-3 h-5 w-5 text-gray-400/80" />
				<div className="flex flex-col ">
					<div className="flex flex-row items-center">
						<div className="font-medium">{props.text}</div>
					</div>
				</div>
			</div>
		</div>
	);
};
