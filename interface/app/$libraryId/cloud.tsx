import { auth, useLibraryMutation, useLibraryQuery } from '@sd/client';
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
	const cloudLibrary = useLibraryQuery(['cloud.library.get'], { suspense: true, retry: false });

	const createLibrary = useLibraryMutation(['cloud.library.create']);

	return (
		<div className="flex flex-row p-4">
			{cloudLibrary.data ? (
				<div className="flex flex-col">
					<p>Library: {cloudLibrary.data.name}</p>
					<p>Instances</p>
					<ul className="space-y-4 pl-4">
						{cloudLibrary.data.instances.map((instance) => (
							<li key={instance.id}>
								<p>Id: {instance.id}</p>
								<p>UUID: {instance.uuid}</p>
								<p>Public Key: {instance.identity}</p>
							</li>
						))}
					</ul>
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
