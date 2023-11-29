import { useBridgeQuery } from '@sd/client';
import { Button } from '@sd/ui';
import { Icon } from '~/components';
import { AuthRequiredOverlay } from '~/components/AuthRequiredOverlay';
import { useRouteTitle } from '~/hooks';

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
	const libraries = useBridgeQuery(['cloud.library.list']);

	if (libraries.isLoading) return <span>Loading...</span>;

	return (
		<>
			{libraries.data?.map((library) => (
				<li key={library.uuid} className="flex flex-row gap-2">
					<span>{library.name}</span>
					<Button
						variant="accent"
						onClick={() => {
							console.log('clicked ', library.name);
						}}
					>
						Join
					</Button>
				</li>
			))}
		</>
	);
}
