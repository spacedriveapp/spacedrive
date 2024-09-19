import { Envelope } from '@phosphor-icons/react';
import { useEffect } from 'react';
import { useBridgeMutation, useBridgeQuery } from '@sd/client';
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

	// Refetch every 10 seconds
	useEffect(() => {
		const interval = setInterval(async () => {
			await devices.refetch();
		}, 10000);
		return () => clearInterval(interval);
	}, []);
	console.log(devices.data);

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
