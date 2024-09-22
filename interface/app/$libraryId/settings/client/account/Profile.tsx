import { Envelope } from '@phosphor-icons/react';
import { useEffect } from 'react';
import {
	SyncGroup,
	SyncGroupWithLibraryAndDevices,
	useBridgeMutation,
	useBridgeQuery,
	useLibraryMutation
} from '@sd/client';
import { Button, Card } from '@sd/ui';
import StatCard from '~/app/$libraryId/overview/StatCard';
import { TruncatedText } from '~/components';
import { hardwareModelToIcon } from '~/util/hardware';

const Profile = ({ email }: { email?: string }) => {
	const emailName = email?.split('@')[0];
	const capitalizedEmailName = (emailName?.charAt(0).toUpperCase() ?? '') + emailName?.slice(1);
	const refreshToken: string =
		JSON.parse(window.localStorage.getItem('frontendCookies') ?? '[]')
			.find((cookie: string) => cookie.startsWith('st-refresh-token'))
			?.split('=')[1]
			.split(';')[0] || '';
	const accessToken: string =
		JSON.parse(window.localStorage.getItem('frontendCookies') ?? '[]')
			.find((cookie: string) => cookie.startsWith('st-access-token'))
			?.split('=')[1]
			.split(';')[0] || '';
	const cloudBootstrap = useBridgeMutation('cloud.bootstrap');
	const cloudDeleteDevice = useBridgeMutation('cloud.devices.delete');
	const devices = useBridgeQuery(['cloud.devices.list', { access_token: accessToken.trim() }]);
	const addLibraryToCloud = useLibraryMutation('cloud.libraries.create');
	const listLibraries = useBridgeQuery([
		'cloud.libraries.list',
		{ access_token: accessToken.trim(), with_device: true }
	]);
	const createSyncGroup = useLibraryMutation('cloud.syncGroups.create');
	const listSyncGroups = useBridgeQuery([
		'cloud.syncGroups.list',
		{ access_token: accessToken.trim(), with_library: true }
	]);
	const requestJoinSyncGroup = useBridgeMutation('cloud.syncGroups.request_join');
	const getGroup = useBridgeQuery([
		'cloud.syncGroups.get',
		{
			access_token: accessToken.trim(),
			pub_id: '0192123b-5d01-7341-aa9d-4a08571052ee',
			with_library: true,
			with_devices: true,
			with_used_storage: true
		}
	]);
	console.log(getGroup.data);
	const currentDevice = useBridgeQuery(['cloud.devices.get_current_device', accessToken.trim()]);
	console.log('Current Device: ', currentDevice.data);

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
			<Card className="relative flex w-full flex-col items-center justify-center !p-0 lg:max-w-[320px]">
				{/* <AuthRequiredOverlay /> */}
				<div className="p-3">
					<h1 className="mx-auto mt-3 text-lg">
						Welcome <span className="font-bold">{capitalizedEmailName},</span>
					</h1>
					<div className="mx-auto mt-4 flex w-full flex-col gap-2">
						<Card className="w-full items-center justify-start gap-1 bg-app-input !px-2">
							<div className="w-[20px]">
								<Envelope weight="fill" width={20} />
							</div>
							<TruncatedText>{email}</TruncatedText>
						</Card>
					</div>
				</div>
			</Card>
			<h2 className="mx-auto mt-4 text-sm">DEBUG</h2>
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
					cloudDeleteDevice.mutate({
						access_token: accessToken.trim(),
						pub_id: '01920812-9bd2-7781-aee5-e19a01497296'
					});
				}}
			>
				Delete Device
			</Button>
			<Button
				className="mt-4 w-full"
				onClick={async () => {
					addLibraryToCloud.mutate(accessToken.trim());
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
					createSyncGroup.mutate(accessToken.trim());
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
						access_token: accessToken.trim(),
						sync_group: getGroup.data! as unknown as SyncGroupWithLibraryAndDevices,
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
					totalSpace={device.storage_size.toString()}
					freeSpace={(device.storage_size - device.used_storage).toString()}
					color="#0362FF"
					connectionType={'cloud'}
				/>
			))}
		</div>
	);
};

export default Profile;
