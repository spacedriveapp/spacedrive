import { useQueryClient } from '@tanstack/react-query';
import { useNavigate } from 'react-router';
import {
	resetOnboardingStore,
	useBridgeMutation,
	useBridgeQuery,
	useLibraryMutation
} from '@sd/client';
import { Button } from '@sd/ui';
import { Icon } from '~/components';
import { AuthRequiredOverlay } from '~/components/AuthRequiredOverlay';
import { useRouteTitle } from '~/hooks';
import { usePlatform } from '~/util/Platform';

import { OnboardingContainer, OnboardingDescription, OnboardingTitle } from './components';

export function JoinLibrary() {
	useRouteTitle('Join Library');

	return (
		<OnboardingContainer>
			<Icon name="Database" size={80} />
			<OnboardingTitle>Join a Library</OnboardingTitle>
			<OnboardingDescription>
				Libraries are a secure, on-device database. Your files remain where they are, the
				Library catalogs them and stores all Spacedrive related data.
			</OnboardingDescription>

			<div className="mt-2">
				<span>Cloud Libraries</span>
				<ul className="relative flex h-32 w-48 flex-col rounded border border-app-frame p-2">
					<CloudLibraries />
					<AuthRequiredOverlay />
				</ul>
			</div>
		</OnboardingContainer>
	);
}

function CloudLibraries() {
	const cloudLibraries = useBridgeQuery(['cloud.library.list']);
	const joinLibrary = useLibraryMutation(['cloud.library.join']);

	const navigate = useNavigate();
	const queryClient = useQueryClient();
	const platform = usePlatform();

	if (cloudLibraries.isLoading) return <span>Loading...</span>;

	return (
		<>
			{cloudLibraries.data?.map((cloudLibrary) => (
				<li key={cloudLibrary.uuid} className="flex flex-row gap-2">
					<span>{cloudLibrary.name}</span>
					<Button
						variant="accent"
						disabled={joinLibrary.isLoading}
						onClick={async () => {
							const library = await joinLibrary.mutateAsync(null);

							queryClient.setQueryData(['library.list'], (libraries: any) => {
								// The invalidation system beat us to it
								if (libraries.find((l: any) => l.uuid === library.uuid))
									return libraries;

								return [...(libraries || []), library];
							});

							platform.refreshMenuBar && platform.refreshMenuBar();

							resetOnboardingStore();
							navigate(`/${library.uuid}`, { replace: true });
						}}
					>
						{joinLibrary.isLoading ? 'Joining...' : 'Join'}
					</Button>
				</li>
			))}
		</>
	);
}
