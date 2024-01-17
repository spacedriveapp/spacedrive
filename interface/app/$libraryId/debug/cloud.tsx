import { auth, useLibraryContext, useLibraryMutation, useLibraryQuery } from '@sd/client';
import { Button } from '@sd/ui';
import { AuthRequiredOverlay } from '~/components/AuthRequiredOverlay';
import { LoginButton } from '~/components/LoginButton';
import { useRouteTitle } from '~/hooks';

export const Component = () => {
	useRouteTitle('Cloud');

	const authState = auth.useStateSnapshot();

	const authSensitiveChild = () => {
		if (authState.status === 'loggedIn') return <Authenticated />;
		if (authState.status === 'notLoggedIn' || authState.status === 'loggingIn')
			return <LoginButton />;

		return null;
	};

	return <div className="flex size-full flex-col items-start p-4">{authSensitiveChild()}</div>;
};

function Authenticated() {
	const { library } = useLibraryContext();

	const cloudLibrary = useLibraryQuery(['cloud.library.get'], { suspense: true, retry: false });

	const createLibrary = useLibraryMutation(['cloud.library.create']);

	const thisInstance = cloudLibrary.data?.instances.find(
		(instance) => instance.uuid === library.instance_id
	);

	return (
		<>
			{cloudLibrary.data ? (
				<div className="flex flex-col items-start space-y-2">
					<div>
						<p>Library</p>
						<p>Name: {cloudLibrary.data.name}</p>
					</div>
					{thisInstance && (
						<div>
							<p>This Instance</p>
							<p>Id: {thisInstance.id}</p>
							<p>UUID: {thisInstance.uuid}</p>
							<p>Public Key: {thisInstance.identity}</p>
						</div>
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
		</>
	);
}
