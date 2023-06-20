import clsx from 'clsx';
import { Repeat, Trash } from 'phosphor-react';
import { useState } from 'react';
import { useNavigate } from 'react-router';
import { Location, Node, arraysEqual, useLibraryMutation, useOnlineLocations } from '@sd/client';
import { Button, Card, Tooltip, dialogManager } from '@sd/ui';
import { Folder } from '~/components/Folder';
import { useIsDark } from '~/hooks';
import DeleteDialog from './DeleteDialog';

interface Props {
	location: Location & { node: Node | null };
}

export default ({ location }: Props) => {
	const navigate = useNavigate();
	const [hide, setHide] = useState(false);

	const fullRescan = useLibraryMutation('locations.fullRescan');
	const onlineLocations = useOnlineLocations();

	const isDark = useIsDark();

	if (hide) return <></>;

	const online = onlineLocations?.some((l) => arraysEqual(location.pub_id, l)) || false;

	return (
		<Card
			className="hover:bg-app-box/70"
			onClick={() => {
				navigate(`${location.id}`);
			}}
		>
			<Folder white={!isDark} className="mr-3 h-10 w-10 self-center" />
			<div className="grid min-w-[110px] grid-cols-1">
				<h1 className="pt-0.5 text-sm font-semibold">{location.name}</h1>
				<p className="mt-0.5 select-text truncate  text-sm text-ink-dull">
					{location.node && (
						<span className="mr-1 rounded bg-app-selected  px-1 py-[1px]">
							{location.node.name}
						</span>
					)}
					{location.path}
				</p>
			</div>
			<div className="flex grow" />
			<div className="flex h-[45px] space-x-2 p-2">
				{/* This is a fake button, do not add disabled prop pls */}

				<Button
					onClick={(e: { stopPropagation: () => void }) => {
						e.stopPropagation();
					}}
					variant="gray"
					className="pointer-events-none flex !px-2 !py-1.5"
				>
					<div
						className={clsx(
							'h-2 w-2  rounded-full',
							online ? 'bg-green-500' : 'bg-red-500'
						)}
					/>
					<span className="ml-1.5 text-xs text-ink-dull">
						{online ? 'Online' : 'Offline'}
					</span>
				</Button>
				<Button
					variant="gray"
					className="!p-1.5"
					onClick={(e: { stopPropagation: () => void }) => {
						e.stopPropagation();
						dialogManager.create((dp) => (
							<DeleteDialog
								{...dp}
								onSuccess={() => setHide(true)}
								locationId={location.id}
							/>
						));
					}}
				>
					<Tooltip label="Delete Location">
						<Trash className="h-4 w-4" />
					</Tooltip>
				</Button>
				<Button
					variant="gray"
					className="!p-1.5"
					onClick={(e: { stopPropagation: () => void }) => {
						e.stopPropagation();
						// this should cause a lite directory rescan, but this will do for now, so the button does something useful
						fullRescan.mutate(location.id);
					}}
				>
					<Tooltip label="Rescan Location">
						<Repeat className="h-4 w-4" />
					</Tooltip>
				</Button>
				{/* <Button variant="gray" className="!p-1.5">
					<CogIcon className="w-4 h-4" />
				</Button> */}
			</div>
		</Card>
	);
};
