import { startTransition } from 'react';
import { auth, useLibraryContext, useLibraryMutation, useLibraryQuery } from '@sd/client';
import { Button } from '@sd/ui';
import { AuthRequiredOverlay } from '~/components/AuthRequiredOverlay';
import { LoginButton } from '~/components/LoginButton';
import { useRouteTitle } from '~/hooks';

export const Component = () => {
	useRouteTitle('Cloud');

	const authState = auth.useStateSnapshot();

	if (authState.status === 'loggedIn') return <Authenticated />;
	if (authState.status === 'notLoggedIn')
		return (
			<div className="flex flex-row p-4">
				<LoginButton />
			</div>
		);

	return null;
};

function Authenticated() {
	const { library } = useLibraryContext();

	const cloudLibrary = useLibraryQuery(['cloud.library.get'], { suspense: true, retry: false });

	const createLibrary = useLibraryMutation(['cloud.library.create']);

	const thisInstance = cloudLibrary.data?.instances.find(
		(instance) => instance.uuid === library.instance_id
	);

	return (
		<div className="flex flex-row p-4">
			{cloudLibrary.data ? (
				<div className="flex flex-col items-start space-y-2">
					<div>
						<p>Library</p>
						<p>Name: {cloudLibrary.data.name}</p>
					</div>
					{thisInstance ? (
						<div>
							<p>This Instance</p>
							<p>Id: {thisInstance.id}</p>
							<p>UUID: {thisInstance.uuid}</p>
							<p>Public Key: {thisInstance.identity}</p>
						</div>
					) : (
						<AddThisInstanceButton />
					)}
					<div>
						<p>Instances</p>
						<ul className="space-y-4 pl-4">
							{cloudLibrary.data.instances
								.filter((instance) => instance.uuid !== library.instance_id)
								.map((instance) => (
									<li key={instance.id}>
										<p>Id: {instance.id}</p>
										<p>UUID: {instance.uuid}</p>
										<p>Public Key: {instance.identity}</p>
									</li>
								))}
						</ul>
					</div>
				</div>
			) : (
				<div className="relative">
					<AuthRequiredOverlay />
					<Button
						disabled={createLibrary.isLoading}
						onClick={() => {
							createLibrary.mutateAsync(null);
						}}
					>
						{createLibrary.isLoading
							? 'Connecting library to Spacedrive Cloud...'
							: 'Connect library to Spacedrive Cloud'}
					</Button>
				</div>
			)}
		</div>
	);
}

function AddThisInstanceButton() {
	const joinCloudLibrary = useLibraryMutation(['cloud.library.join']);

	return (
		<Button
			variant="accent"
			disabled={joinCloudLibrary.isLoading || joinCloudLibrary.isSuccess}
			onClick={() => joinCloudLibrary.mutate(null)}
		>
			Add This Instance To Cloud
		</Button>
	);
}
