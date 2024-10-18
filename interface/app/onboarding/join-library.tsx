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
import { useLocale, useRouteTitle } from '~/hooks';
import { usePlatform } from '~/util/Platform';

import { OnboardingContainer, OnboardingDescription, OnboardingTitle } from './components';

export function JoinLibrary() {
	const { t } = useLocale();

	useRouteTitle('Join Library');

	return (
		<OnboardingContainer>
			<Icon name="Database" size={80} />
			<OnboardingTitle>{t('join_library')}</OnboardingTitle>
			<OnboardingDescription>{t('join_library_description')}</OnboardingDescription>

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
	const { t } = useLocale();

	const cloudLibraries = useBridgeQuery(['cloud.library.list']);
	const joinLibrary = useBridgeMutation(['cloud.library.join']);

	const navigate = useNavigate();
	const queryClient = useQueryClient();
	const platform = usePlatform();

	if (cloudLibraries.isLoading) return <span>{t('loading')}...</span>;

	return (
		<>
			{cloudLibraries.data?.map((cloudLibrary) => (
				<li key={cloudLibrary.uuid} className="flex flex-row gap-2">
					<span>{cloudLibrary.name}</span>
					<Button
						variant="accent"
						disabled={joinLibrary.isPending}
						onClick={async () => {
							const library = await joinLibrary.mutateAsync(cloudLibrary.uuid);

							queryClient.setQueryData(['library.list'], (libraries: any) => {
								// The invalidation system beat us to it
								if ((libraries || []).find((l: any) => l.uuid === library.uuid))
									return libraries;

								return [...(libraries || []), library];
							});

							if (platform.refreshMenuBar) platform.refreshMenuBar();

							resetOnboardingStore();
							navigate(`/${library.uuid}`, { replace: true });
						}}
					>
						{joinLibrary.isPending && joinLibrary.variables === cloudLibrary.uuid
							? 'Joining...'
							: 'Join'}
					</Button>
				</li>
			))}
		</>
	);
}
