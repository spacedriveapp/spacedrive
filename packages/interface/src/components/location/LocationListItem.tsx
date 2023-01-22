import { useLibraryMutation } from '@sd/client';
import { Location, Node } from '@sd/client';
import { Button, Card, Dialog } from '@sd/ui';
import clsx from 'clsx';
import { Repeat, Trash } from 'phosphor-react';
import { useState } from 'react';
import { useNavigate } from 'react-router';

import { Folder } from '../icons/Folder';

import { useZodForm, z } from '@sd/ui/src/forms';

interface LocationListItemProps {
	location: Location & { node: Node };
}

export default function LocationListItem({ location }: LocationListItemProps) {
	const navigate = useNavigate();
	const [hide, setHide] = useState(false);
	const [open, setOpen] = useState(false);

	const { mutate: locRescan } = useLibraryMutation('locations.fullRescan');

	const { mutate: deleteLoc, isLoading: locDeletePending } = useLibraryMutation(
		'locations.delete',
		{
			onSuccess: () => {
				setHide(true);
			}
		}
	);

	const form = useZodForm({ schema: z.object({}) });

	if (hide) return <></>;

	return (
		<Card
			className="cursor-pointer hover:bg-app-box/70"
			onClick={() => {
				navigate(`/settings/locations/location/${location.id}`);
			}}
		>
			<Folder size={30} className="mr-3" />
			<div className="grid grid-cols-1 min-w-[110px]">
				<h1 className="pt-0.5 text-sm font-semibold">{location.name}</h1>
				<p className="mt-0.5 text-sm truncate  select-text text-ink-dull">
					<span className="py-[1px] px-1 bg-app-selected  rounded mr-1">{location.node.name}</span>
					{location.local_path}
				</p>
			</div>
			<div className="flex flex-grow" />
			<div className="flex h-[45px] p-2 space-x-2">
				{/* This is a fake button, do not add disabled prop pls */}
				<Button
					onClick={(e: { stopPropagation: () => void }) => {
						e.stopPropagation();
					}}
					variant="gray"
					className="!py-1.5 !px-2 pointer-events-none flex"
				>
					<div
						className={clsx(
							'w-2 h-2  rounded-full',
							location.is_online ? 'bg-green-500' : 'bg-red-500'
						)}
					/>
					<span className="ml-1.5 text-xs text-ink-dull">
						{location.is_online ? 'Online' : 'Offline'}
					</span>
				</Button>
				<Dialog
					form={form}
					onSubmit={form.handleSubmit(() => {
						deleteLoc(location.id);
					})}
					open={open}
					setOpen={setOpen}
					title="Delete Location"
					description="Deleting a location will also remove all files associated with it from the Spacedrive database, the files themselves will not be deleted."
					loading={locDeletePending}
					ctaDanger
					ctaLabel="Delete"
					trigger={
						<Button
							onClick={(e: { stopPropagation: () => void }) => {
								e.stopPropagation();
							}}
							variant="gray"
							className="!p-1.5"
						>
							<Trash className="w-4 h-4" />
						</Button>
					}
				/>
				<Button
					variant="gray"
					className="!p-1.5"
					onClick={(e: { stopPropagation: () => void }) => {
						e.stopPropagation();
						// this should cause a lite directory rescan, but this will do for now, so the button does something useful
						locRescan(location.id);
					}}
				>
					<Repeat className="w-4 h-4" />
				</Button>
				{/* <Button variant="gray" className="!p-1.5">
					<CogIcon className="w-4 h-4" />
				</Button> */}
			</div>
		</Card>
	);
}
