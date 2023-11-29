import { useBridgeQuery, useLibraryContext, useLibraryMutation, useLibraryQuery } from '@sd/client';
import { Button } from '@sd/ui';
import { AuthRequiredOverlay } from '~/components/AuthRequiredOverlay';
import { LoginButton } from '~/components/LoginButton';
import { useRouteTitle } from '~/hooks';

export const Component = () => {
	useRouteTitle('Cloud');

	const me = useBridgeQuery(['auth.me'], { retry: false });

	return me.data ? (
		<Authenticated />
	) : (
		<div className="flex flex-row p-4">
			<LoginButton />
		</div>
	);
};

function Authenticated() {
	const { library } = useLibraryContext();

	const cloudLibrary = useLibraryQuery(['cloud.library.get'], { suspense: true, retry: false });

	const createLibrary = useLibraryMutation(['cloud.library.create']);

	const instanceIsConnected = cloudLibrary.data?.instances.some(
		(i) => i.uuid === library.instance_id
	);

	return (
		<div className="flex flex-row p-4">
			{cloudLibrary.data ? (
				<div className="flex flex-col">
					<p>Name: {cloudLibrary.data.name}</p>
					<p>Instances</p>
					<ul className="space-y-4 pl-4">
						{cloudLibrary.data.instances.map((instance) => (
							<li key={instance.id}>
								<p>Id: {instance.id}</p>
								<p>UUID: {instance.uuid}</p>
							</li>
						))}
					</ul>
					{!instanceIsConnected && (
						<div>
							<span>Join Library:</span>
							<ConnectLibrary />
						</div>
					)}
				</div>
			) : (
				<div className="relative">
					<AuthRequiredOverlay />
					<Button
						onClick={() => {
							createLibrary.mutateAsync(null);
						}}
					>
						Connect library to Spacedrive Cloud
					</Button>
				</div>
			)}
		</div>
	);
}

function ConnectLibrary() {
	const libraries = useBridgeQuery(['cloud.library.list']);

	const connectLibrary = useLibraryMutation(['cloud.library.connect']);

	return (
		<ul>
			{libraries.data?.map((library) => (
				<li key={library.name} className="flex flex-row gap-2 p-2">
					<span>{library.name}</span>
					<Button
						variant="accent"
						onClick={() => {
							connectLibrary.mutate(library.uuid);
						}}
					>
						Connect
					</Button>
				</li>
			))}
		</ul>
	);
}
