import { Envelope, User } from '@phosphor-icons/react';
import { useEffect, useState } from 'react';
import { auth, useBridgeMutation, useBridgeQuery, useFeatureFlag } from '@sd/client';
import { Button, Card, Input, toast } from '@sd/ui';
import { TruncatedText } from '~/components';
import { AuthRequiredOverlay } from '~/components/AuthRequiredOverlay';
import { useLocale } from '~/hooks';

import { Heading } from '../Layout';

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
				<Profile authStore={authStore} email={me.data?.email} />
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
				className="flex size-[90px] items-center justify-center rounded-full
	 border border-app-line bg-app-input"
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
	const { t } = useLocale();
	const doTheThing = useBridgeMutation('cloud.locations.testing', {
		onSuccess() {
			toast.success(t('uploaded_file'));
		},
		onError(err) {
			toast.error(t('error_message', { error: err.message }));
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
								placeholder={t('my_sick_location')}
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
								{t('create_location')}
							</Button>
						</div>
					</div>
				}
				title={t('hosted_locations')}
				description={t('hosted_locations_description')}
			/>

			{/* TODO: Cleanup this mess + styles */}
			{locations.status === 'loading' ? <div>{`${t('loading')!}`}</div> : null}
			{locations.status !== 'loading' && locations.data?.length === 0 ? (
				<div>{t('dont_have_any')}</div>
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
								{t('delete')}
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
								{t('do_the_thing')}
							</Button>
						</div>
					))}
				</div>
			)}

			<div>
				<p>{t('path_to_save_do_the_thing')}</p>
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
