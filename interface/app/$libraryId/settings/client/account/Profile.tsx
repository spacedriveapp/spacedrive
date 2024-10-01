import { Envelope } from '@phosphor-icons/react';
import { Dispatch, SetStateAction, useEffect } from 'react';
import {
	CloudSyncGroupWithLibraryAndDevices,
	useBridgeMutation,
	useBridgeQuery,
	useLibraryMutation
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
	// console.log(accessToken);
	const cloudBootstrap = useBridgeMutation('cloud.bootstrap');
	const cloudDeleteDevice = useBridgeMutation('cloud.devices.delete');
	const devices = useBridgeQuery(['cloud.devices.list']);
	const addLibraryToCloud = useLibraryMutation('cloud.libraries.create');
	const listLibraries = useBridgeQuery(['cloud.libraries.list', true]);
	const createSyncGroup = useLibraryMutation('cloud.syncGroups.create');
	const listSyncGroups = useBridgeQuery(['cloud.syncGroups.list']);
	const requestJoinSyncGroup = useBridgeMutation('cloud.syncGroups.request_join');
	const getGroup = useBridgeQuery([
		'cloud.syncGroups.get',
		{
			pub_id: '019237a1-586c-7651-afd3-525047b02375',
			kind: 'WithDevices'
		}
	]);
	const currentDevice = useBridgeQuery(['cloud.devices.get_current_device']);
	// console.log('Current Device: ', currentDevice.data);

	// Refetch every 10 seconds
	useEffect(() => {
		const interval = setInterval(async () => {
			await devices.refetch();
		}, 10000);
		return () => clearInterval(interval);
	}, []);
	// console.log(devices.data);

	return (
		<div className="flex flex-col gap-5">
			{/* Top Section with Welcome and Logout */}
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
						{/* Account Stats (for future services) */}
						{/* <p className="font-medium">Roles:</p> // We don't use roles, at least currently.
						<div className="flex flex-wrap gap-2">
							{user.roles.map((role) => (
								<Pill key={role}>{role.toLocaleUpperCase()}</Pill>
							))}
						</div> */}
					</div>
				</Card>
			</div>

			{/* MT is added to hide */}
			<h2 className="mx-auto mt-80 text-sm">DEBUG</h2>
			<Button
				className="mt-4 w-full"
				onClick={async () => {
					cloudBootstrap.mutate([accessToken.trim(), refreshToken.trim()]);
				}}
			>
				Start Cloud Bootstrap
			</Button>
			<Button
				className="mt-4 w-full"
				onClick={async () => {
					addLibraryToCloud.mutate(null);
				}}
			>
				Add Library to Cloud
			</Button>
			<Button
				className="mt-4 w-full"
				onClick={async () => {
					listLibraries.refetch();
					console.log(listLibraries.data);
				}}
			>
				List Libraries
			</Button>

			<Button
				className="mt-4 w-full"
				onClick={async () => {
					createSyncGroup.mutate(null);
				}}
			>
				Create Sync Group
			</Button>
			<Button
				className="mt-4 w-full"
				onClick={async () => {
					listSyncGroups.refetch();
					console.log(listSyncGroups.data);
				}}
			>
				List Sync Groups
			</Button>
			<Button
				className="mt-4 w-full"
				onClick={async () => {
					requestJoinSyncGroup.mutate({
						sync_group: (
							getGroup.data! as unknown as {
								WithDevices: CloudSyncGroupWithLibraryAndDevices;
							}
						).WithDevices,
						asking_device: currentDevice.data!
					});
				}}
			>
				Request Join Sync Group
			</Button>
			{/* List all devices from const devices */}
			{devices.data?.map((device) => (
				// <Card
				// 	key={device.pub_id}
				// 	className="w-full items-center justify-start gap-1 bg-app-input !px-2"
				// >

				// </Card>
				<StatCard
					key={device.pub_id}
					name={device.name}
					// TODO (Optional): Use Brand Type for Different Android Models/iOS Models using DeviceInfo.getBrand()
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
