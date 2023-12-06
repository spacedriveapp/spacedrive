import { Envelope, User } from '@phosphor-icons/react';
import { iconNames } from '@sd/assets/util';
import { useEffect, useState } from 'react';
import { auth, useBridgeMutation, useBridgeQuery, useFeatureFlag } from '@sd/client';
import { Button, Card, Input, toast } from '@sd/ui';
import { Icon, TruncatedText } from '~/components';
import { AuthRequiredOverlay } from '~/components/AuthRequiredOverlay';

import { Heading } from '../Layout';

export const Component = () => {
	const me = useBridgeQuery(['auth.me'], { retry: false });
	const authStore = auth.useStateSnapshot();
	return (
		<>
			<Heading
				rightArea={
					<>
						{authStore.status === 'loggedIn' && (
							<div className="flex-row space-x-2">
								<Button variant="accent" size="sm" onClick={auth.logout}>
									Logout
								</Button>
							</div>
						)}
					</>
				}
				title="Your account"
				description="Spacedrive account and information."
			/>
			<div className="flex flex-col justify-between gap-5 lg:flex-row">
				<Profile authStore={authStore} email={me.data?.email} />
				<Cloud />
			</div>
			{useFeatureFlag('hostedLocations') && <HostedLocationsPlayground />}
		</>
	);
};

const Profile = ({ email, authStore }: { email?: string; authStore: { status: string } }) => {
	const emailName = authStore.status === 'loggedIn' ? email?.split('@')[0] : 'guest user';
	return (
		<Card className="relative flex w-full flex-col items-center justify-center !p-6 lg:max-w-[320px]">
			<AuthRequiredOverlay />
			<div
				className="flex h-[90px] w-[90px] items-center justify-center
	 rounded-full border border-app-line bg-app-input"
			>
				<User weight="fill" className="mx-auto text-4xl text-ink-faint" />
			</div>
			<h1 className="mx-auto mt-3 text-lg">
				Welcome <span className="font-bold">{emailName},</span>
			</h1>
			<div className="mx-auto mt-4 flex w-full flex-col gap-2">
				<Card className="w-full items-center justify-start gap-1 bg-app-input !px-2">
					<div className="w-[20px]">
						<Envelope weight="fill" width={20} />
					</div>
					<TruncatedText>
						{authStore.status === 'loggedIn' ? email : 'guestuser@outlook.com'}
					</TruncatedText>
				</Card>
			</div>
		</Card>
	);
};

const services: { service: string; icon: keyof typeof iconNames }[] = [
	{ service: 'S3', icon: 'AmazonS3' },
	{ service: 'Dropbox', icon: 'Dropbox' },
	{ service: 'DAV', icon: 'DAV' },
	{ service: 'Mega', icon: 'Mega' },
	{ service: 'Onedrive', icon: 'OneDrive' },
	{ service: 'Google Drive', icon: 'GoogleDrive' }
];
const Cloud = () => {
	return (
		<Card className="flex w-full flex-col !p-6">
			<h1 className="text-lg font-bold">Cloud services</h1>
			<div className="mt-5 grid grid-cols-1 gap-2 lg:grid-cols-3">
				{services.map((s, index) => (
					<Card
						key={index}
						className="relative flex flex-col items-center justify-center gap-2 bg-app-input !p-4"
					>
						<div
							className="z-5 absolute flex h-full w-full items-center justify-center rounded-md bg-app/50 backdrop-blur-[8px]"
							key={index}
						>
							<p className="text-center text-[13px] font-medium text-ink-faint">
								Coming soon
							</p>
						</div>
						<Icon name={s.icon} size={50} />
						<p className="text-[14px] font-medium text-ink">{s.service}</p>
					</Card>
				))}
			</div>
		</Card>
	);
};

function HostedLocationsPlayground() {
	const locations = useBridgeQuery(['cloud.locations.list'], { retry: false });

	const [locationName, setLocationName] = useState('');
	const [path, setPath] = useState('');
	const createLocation = useBridgeMutation('cloud.locations.create', {
		onSuccess(data) {
			// console.log('DATA', data); // TODO: Optimistic UI

			locations.refetch();
			setLocationName('');
		}
	});
	const removeLocation = useBridgeMutation('cloud.locations.remove', {
		onSuccess() {
			// TODO: Optimistic UI

			locations.refetch();
		}
	});
	const doTheThing = useBridgeMutation('cloud.locations.testing', {
		onSuccess() {
			toast.success('Uploaded file!');
		},
		onError(err) {
			toast.error(err.message);
		}
	});

	useEffect(() => {
		if (path === '' && locations.data?.[0]) {
			setPath(`location/${locations.data[0].id}/hello.txt`);
		}
	}, [path, locations.data]);

	const isLoading = createLocation.isLoading || removeLocation.isLoading || doTheThing.isLoading;

	return (
		<>
			<Heading
				rightArea={
					<div className="flex-row space-x-2">
						{/* TODO: We need UI for this. I wish I could use `prompt` for now but Tauri doesn't have it :( */}
						<div className="flex flex-row space-x-4">
							<Input
								className="grow"
								value={locationName}
								onInput={(e) => setLocationName(e.currentTarget.value)}
								placeholder="My sick location"
								disabled={isLoading}
							/>

							<Button
								variant="accent"
								size="sm"
								onClick={() => {
									if (locationName === '') return;
									createLocation.mutate(locationName);
								}}
								disabled={isLoading}
							>
								Create Location
							</Button>
						</div>
					</div>
				}
				title="Hosted Locations"
				description="Augment your local storage with our cloud!"
			/>

			{/* TODO: Cleanup this mess + styles */}
			{locations.status === 'loading' ? <div>Loading!</div> : null}
			{locations.status !== 'loading' && locations.data?.length === 0 ? (
				<div>Looks like you don't have any!</div>
			) : (
				<div>
					{locations.data?.map((location) => (
						<div key={location.id} className="flex flex-row space-x-5">
							<h1>{location.name}</h1>
							<Button
								variant="accent"
								size="sm"
								onClick={() => removeLocation.mutate(location.id)}
								disabled={isLoading}
							>
								Delete
							</Button>
							<Button
								variant="accent"
								size="sm"
								onClick={() =>
									doTheThing.mutate({
										id: location.id,
										path
									})
								}
								disabled={isLoading}
							>
								Do the thing
							</Button>
						</div>
					))}
				</div>
			)}

			<div>
				<p>Path to save when clicking 'Do the thing':</p>
				<Input
					className="grow"
					value={path}
					onInput={(e) => setPath(e.currentTarget.value)}
					disabled={isLoading}
				/>
			</div>
		</>
	);
}
