import { Repeat, Trash } from '@phosphor-icons/react';
import clsx from 'clsx';
import { useState } from 'react';
import { useNavigate } from 'react-router';
import {
	arraysEqual,
	humanizeSize,
	Location,
	useLibraryMutation,
	useOnlineLocations
} from '@sd/client';
import { Button, buttonStyles, Card, dialogManager, Tooltip } from '@sd/ui';
import { Icon } from '~/components';
import { useLocale } from '~/hooks';

import DeleteDialog from './DeleteDialog';

interface Props {
	location: Location;
}

export default ({ location }: Props) => {
	const navigate = useNavigate();
	const [hide, setHide] = useState(false);

	const { t } = useLocale();

	const fullRescan = useLibraryMutation('locations.fullRescan');
	const onlineLocations = useOnlineLocations();

	if (hide) return <></>;

	const online = onlineLocations.some((l) => arraysEqual(location.pub_id, l));

	return (
		<Card
			className="hover:bg-app-box/70"
			onClick={() => {
				navigate(`${location.id}`);
			}}
		>
			<Icon size={24} name="Folder" className="mr-3 size-10 self-center" />
			<div className="grid min-w-[110px] grid-cols-1">
				<h1 className="truncate pt-0.5 text-sm font-semibold">{location.name}</h1>

				<p className="mt-0.5 select-text truncate text-sm text-ink-dull">
					{/* // TODO: This is ephemeral so it should not come from the DB. Eg. a external USB can move between nodes */}
					{/* {location.node && (
						<span className="mr-1 rounded bg-app-selected  px-1 py-[1px]">
							{location.node.name}
						</span>
					)} */}
					{location.path}
				</p>
			</div>
			<div className="flex grow" />
			<div className="flex h-[45px] w-full max-w-fit space-x-2 p-2">
				<Tooltip position="top" label={t('size')}>
					<div
						className={buttonStyles({
							variant: 'gray',
							className: 'pointer-events-none'
						})}
						onClick={(e: { stopPropagation: () => void }) => {
							e.stopPropagation();
						}}
					>
						<span className="max-w-[34px] truncate text-xs text-ink-dull">
							{humanizeSize(location.size_in_bytes).value}
						</span>
						<span className="ml-px text-[10px] text-ink-dull/60">
							{t(`size_${humanizeSize(location.size_in_bytes).unit.toLowerCase()}`)}
						</span>
					</div>
				</Tooltip>
				{/* This is a fake button, do not add disabled prop pls */}
				<Tooltip
					position="top"
					className="flex"
					tooltipClassName="max-w-[140px]"
					label={
						online
							? t('location_connected_tooltip')
							: t('location_disconnected_tooltip')
					}
				>
					<div
						className={buttonStyles({
							variant: 'gray',
							className: 'pointer-events-none flex'
						})}
					>
						<div
							className={clsx(
								'size-2 rounded-full',
								online ? 'bg-green-500' : 'bg-red-500'
							)}
						/>
						<span className="ml-1.5 truncate text-xs text-ink-dull">
							{online ? t('connected') : t('disconnected')}
						</span>
					</div>
				</Tooltip>
				<Tooltip position="top" label={t('delete')}>
					<Button
						variant="gray"
						className="h-full !p-1.5"
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
						<Trash className="size-4" />
					</Button>
				</Tooltip>
				<Tooltip position="top" label={t('rescan')}>
					<Button
						variant="gray"
						className="h-full !p-1.5"
						onClick={(e: { stopPropagation: () => void }) => {
							e.stopPropagation();
							// this should cause a lite directory rescan, but this will do for now, so the button does something useful
							fullRescan.mutate({
								location_id: location.id,
								reidentify_objects: false
							});
						}}
					>
						<Repeat className="size-4" />
					</Button>
				</Tooltip>
				{/* <Button variant="gray" className="!p-1.5">
					<CogIcon className="w-4 h-4" />
				</Button> */}
			</div>
		</Card>
	);
};
