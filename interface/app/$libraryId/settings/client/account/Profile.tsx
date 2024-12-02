import { Envelope } from '@phosphor-icons/react';
import clsx from 'clsx';
import { Dispatch, SetStateAction, useEffect, useState } from 'react';
import {
	SyncStatus,
	useBridgeMutation,
	useBridgeQuery,
	useBridgeSubscription,
	useLibraryMutation,
	useLibrarySubscription
} from '@sd/client';
import { Button, Card, tw } from '@sd/ui';
import StatCard from '~/app/$libraryId/overview/StatCard';
import { TruncatedText } from '~/components';
import { getTokens } from '~/util';
import { hardwareModelToIcon } from '~/util/hardware';

type User = {
	email: string;
	id: string;
	timejoined: number;
	roles: string[];
};

const Pill = tw.div`px-1.5 py-[1px] rounded text-tiny font-medium text-ink-dull bg-app-box border border-app-line`;

const Profile = ({
	user,
	setReload
}: {
	user: User;
	setReload: Dispatch<SetStateAction<boolean>>;
}) => {
	const emailName = user.email?.split('@')[0];
	const capitalizedEmailName = (emailName?.charAt(0).toUpperCase() ?? '') + emailName?.slice(1);
	const { accessToken, refreshToken } = getTokens();

	const cloudBootstrap = useBridgeMutation('cloud.bootstrap');
	const devices = useBridgeQuery(['cloud.devices.list']);
	const addLibraryToCloud = useLibraryMutation('cloud.libraries.create');
	const [syncStatus, setSyncStatus] = useState<SyncStatus | null>(null);
	useLibrarySubscription(['sync.active'], {
		onData: (data) => {
			console.log('sync activity', data);
			setSyncStatus(data);
		}
	});
	const listLibraries = useBridgeQuery(['cloud.libraries.list', true]);
	const createSyncGroup = useLibraryMutation('cloud.syncGroups.create');
	const listSyncGroups = useBridgeQuery(['cloud.syncGroups.list']);
	const requestJoinSyncGroup = useBridgeMutation('cloud.syncGroups.request_join');
	const currentDevice = useBridgeQuery(['cloud.devices.get_current_device']);
	const hasBootstrapped = useBridgeQuery(['cloud.hasBootstrapped']);

	// Refetch libraries and devices every 10 seconds
	useEffect(() => {
		const interval = setInterval(async () => {
			await devices.refetch();
			await listLibraries.refetch();
		}, 10000);
		return () => clearInterval(interval);
	}, [devices, listLibraries]);

	return (
		<div className="flex flex-col gap-5">
			{/* Top Section with Profile Information */}
			<div className="flex w-full items-start justify-between">
				<Card className="relative flex w-full flex-col items-start justify-start space-y-4 !p-5 lg:max-w-[320px]">
					<div>
						<h2 className="text-md text-lg font-semibold">Profile Information</h2>
						<div className="mt-1 flex items-center gap-1 rounded-md bg-app-input px-3 py-2">
							<Envelope weight="fill" width={20} />
							<TruncatedText>{user.email}</TruncatedText>
						</div>
					</div>
					<div className="flex flex-col gap-3">
						<div>
							<p className="font-medium">Joined on</p>
							<p className="text-ink-dull">
								{new Date(user.timejoined).toLocaleDateString()}
							</p>
						</div>
						<div>
							<p className="font-medium">User ID</p>
							<p className="text-ink-dull">{user.id}</p>
						</div>
					</div>
				</Card>
			</div>

			{/* Sync activity */}
			<div className="mt-5 flex flex-col">
				<h2 className="text-md mb-2 font-semibold">Sync Activity</h2>
				<div className="flex flex-row gap-2">
					{Object.keys(syncStatus ?? {}).map((status, index) => (
						<Card key={index} className="flex w-full items-center p-4">
							<div
								className={clsx(
									'mr-2 size-[15px] rounded-full bg-app-box',
									syncStatus?.[status as keyof SyncStatus]
										? 'bg-accent'
										: 'bg-app-input'
								)}
							/>
							<h3 className="text-sm font-semibold">{status}</h3>
						</Card>
					))}
				</div>
			</div>

			{/* Automatically list libraries */}
			<div className="mt-5 flex flex-col gap-3">
				<h2 className="text-md font-semibold">Cloud Libraries</h2>
				{listLibraries.data?.map((library) => (
					<Card key={library.pub_id} className="w-full p-4">
						<h3 className="text-sm font-semibold">{library.name}</h3>
					</Card>
				)) || <p>No libraries found.</p>}
			</div>

			{/* Debug Buttons */}
			<div className="flex gap-2">
				{!hasBootstrapped.data && (
					<Button
						variant="gray"
						onClick={async () => {
							cloudBootstrap.mutate([accessToken.trim(), refreshToken.trim()]);
						}}
					>
						Start Cloud Bootstrap
					</Button>
				)}
				<Button
					variant="gray"
					onClick={async () => {
						addLibraryToCloud.mutate(null);
					}}
				>
					Add Library to Cloud
				</Button>
				<Button
					variant="gray"
					onClick={async () => {
						createSyncGroup.mutate(null);
					}}
				>
					Create Sync Group
				</Button>
			</div>

			{/* Automatically list sync groups and provide a join button */}
			<div className="mt-5 flex flex-col gap-3">
				<h2 className="text-md font-semibold">Library Sync Groups</h2>
				{listSyncGroups.data?.map((group) => (
					<Card key={group.pub_id} className="w-full p-4">
						<h3 className="text-sm font-semibold">{group.library.name}</h3>
						<Button
							className="mt-2"
							onClick={async () => {
								if (!currentDevice.data) await currentDevice.refetch();
								if (currentDevice.data && devices.data) {
									requestJoinSyncGroup.mutate({
										asking_device: currentDevice.data,
										sync_group: {
											devices: devices.data,
											...group
										}
									});
								}
							}}
						>
							Join Sync Group
						</Button>
					</Card>
				)) || <p>No sync groups found.</p>}
			</div>

			{/* List all devices from const devices */}
			{devices.data?.map((device) => (
				<StatCard
					key={device.pub_id}
					name={device.name}
					icon={hardwareModelToIcon(device.hardware_model)}
					totalSpace={'0'}
					freeSpace={'0'}
					color="#0362FF"
					connectionType={'cloud'}
				/>
			))}
		</div>
	);
};

export default Profile;
