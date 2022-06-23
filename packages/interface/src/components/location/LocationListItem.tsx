import { RefreshIcon } from '@heroicons/react/outline';
import { CogIcon, TrashIcon } from '@heroicons/react/solid';
import { command, useBridgeCommand } from '@sd/client';
import { LocationResource } from '@sd/core';
import { Button } from '@sd/ui';
import clsx from 'clsx';
import React, { useState } from 'react';

import { Folder } from '../icons/Folder';
import Dialog from '../layout/Dialog';

interface LocationListItemProps {
	location: LocationResource;
}

export default function LocationListItem({ location }: LocationListItemProps) {
	const [hide, setHide] = useState(false);

	const { mutate: deleteLoc, isLoading: locDeletePending } = useBridgeCommand('LocDelete', {
		onSuccess: () => {
			setHide(true);
		}
	});

	if (hide) return <></>;

	return (
		<div className="flex w-full px-4 py-2 border border-gray-500 rounded-lg bg-gray-550">
			<Folder size={30} className="mr-3" />
			<div className="flex flex-col">
				<h1 className="pt-0.5 text-sm font-semibold">{location.name}</h1>
				<p className="mt-1 text-sm select-text text-gray-250">
					<span className="py-0.5 px-1 bg-gray-500 rounded mr-1">{location.node?.name}</span>
					{location.path}
				</p>
			</div>
			<div className="flex flex-grow" />
			<div className="flex h-[45px] p-2 space-x-2">
				<Button disabled variant="gray" className="!p-1.5 pointer-events-none flex">
					<>
						<div
							className={clsx(
								'w-2 h-2  rounded-full',
								location.is_online ? 'bg-green-500' : 'bg-red-500'
							)}
						/>
						<span className="ml-1.5 text-xs text-gray-350">
							{location.is_online ? 'Online' : 'Offline'}
						</span>
					</>
				</Button>
				<Dialog
					title="Delete Location"
					description="Deleting a location will also remove all files associated with it from the Spacedrive database, the files themselves will not be deleted."
					ctaAction={() => {
						deleteLoc({ id: location.id });
					}}
					loading={locDeletePending}
					ctaDanger
					ctaLabel="Delete"
					trigger={
						<Button variant="gray" className="!p-1.5">
							<TrashIcon className="w-4 h-4" />
						</Button>
					}
				/>
				<Button variant="gray" className="!p-1.5">
					<RefreshIcon className="w-4 h-4" />
				</Button>
				{/* <Button variant="gray" className="!p-1.5">
					<CogIcon className="w-4 h-4" />
				</Button> */}
			</div>
		</div>
	);
}
