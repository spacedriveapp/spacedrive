import clsx from 'clsx';
import { Repeat, Trash } from 'phosphor-react';
import { useState } from 'react';
import { useNavigate } from 'react-router';
import { arraysEqual, useLibraryMutation, useOnlineLocations } from '@sd/client';
import { Location, Node } from '@sd/client';
import { Button, Card, Tooltip, dialogManager } from '@sd/ui';
import { Folder } from '~/components/Folder';
import DeleteDialog from './DeleteDialog';

interface Props {
	location: Location & { node: Node };
}

export default ({ location }: Props) => {
	const navigate = useNavigate();
	const [hide, setHide] = useState(false);

	const fullRescan = useLibraryMutation('locations.fullRescan');
	const onlineLocations = useOnlineLocations();

	if (hide) return <></>;

	const online = onlineLocations?.some((l) => arraysEqual(location.pub_id, l)) || false;

	return (
		<Card
			className="hover:bg-app-box/70"
			onClick={() => {
				navigate(`${location.id}`);
			}}
		>
			<Folder size={30} className="mr-3" />
			<div className="grid min-w-[110px] grid-cols-1">
				<h1 className="pt-0.5 text-sm font-semibold">{location.name}</h1>
				<p className="text-ink-dull mt-0.5 select-text  truncate text-sm">
					<span className="bg-app-selected mr-1 rounded  py-[1px] px-1">{location.node.name}</span>
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
					className="pointer-events-none flex !py-1.5 !px-2"
				>
					<div className={clsx('h-2 w-2  rounded-full', online ? 'bg-green-500' : 'bg-red-500')} />
					<span className="text-ink-dull ml-1.5 text-xs">{online ? 'Online' : 'Offline'}</span>
				</Button>
				<Button
					variant="gray"
					className="!p-1.5"
					onClick={(e: { stopPropagation: () => void }) => {
						e.stopPropagation();
						dialogManager.create((dp) => (
							<DeleteDialog {...dp} onSuccess={() => setHide(true)} locationId={location.id} />
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
