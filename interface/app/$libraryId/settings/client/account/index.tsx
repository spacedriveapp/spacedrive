import { auth, useBridgeMutation, useBridgeQuery, useFeatureFlag } from '@sd/client';
import { Button, Input, toast } from '@sd/ui';
import { useEffect, useState } from 'react';
import { useLocale } from '~/hooks';

import { Heading } from '../../Layout';
import LoginRegister from './LoginRegister';
import Profile from './Profile';

export const Component = () => {
	const { t } = useLocale();
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
									{t('logout')}
								</Button>
							</div>
						)}
					</>
				}
				title={t('spacedrive_cloud')}
				description={t('spacedrive_cloud_description')}
			/>
			<div className="flex flex-col justify-between gap-5 lg:flex-row">
				{authStore.status === 'notLoggedIn' ? (
					<LoginRegister/>
				) : (
					<Profile authStore={authStore} email={me.data?.email} />
				)}
			</div>
			{useFeatureFlag('hostedLocations') && <HostedLocationsPlayground />}
		</>
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
