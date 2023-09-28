import { useQueryClient } from '@tanstack/react-query';
import { useRef, useState } from 'react';
import { useBridgeMutation, useBridgeQuery, useBridgeSubscription } from '@sd/client';
import { Button, Card } from '@sd/ui';
import { usePlatform } from '~/util/Platform';

type State = { status: 'Idle' } | { status: 'LoggingIn' };

export function LoginButton({ onLogin }: { onLogin: () => void }) {
	const [state, setState] = useState<State>({ status: 'Idle' });

	const platform = usePlatform();

	const ret = useRef(null);

	useBridgeSubscription(['auth.loginSession'], {
		enabled: state.status === 'LoggingIn',
		onData(data) {
			if (data === 'Complete') {
				onLogin();
				platform.auth.finish?.(ret.current);
			} else if (data === 'Error') setState({ status: 'Idle' });
			else {
				ret.current = platform.auth.start(data.Start.verification_url_complete);
			}
		},
		onError() {
			setState({ status: 'Idle' });
		}
	});

	return (
		<Button
			variant="accent"
			disabled={state.status !== 'Idle'}
			onClick={() => setState({ status: 'LoggingIn' })}
		>
			{state.status === 'Idle' ? 'Login' : 'Logging In...'}
		</Button>
	);
}

export function SpacedriveAccount() {
	const user = useBridgeQuery(['auth.me']);

	const logout = useBridgeMutation(['auth.logout']);

	const queryClient = useQueryClient();

	return (
		<Card className="px-5">
			<div className="my-2 flex w-full flex-col">
				<div className="flex flex-row items-center justify-between">
					<span className="font-semibold">Spacedrive Account</span>
					{user.isFetchedAfterMount ? (
						user.data ? (
							<Button
								variant="outline"
								onClick={async () => {
									await logout.mutateAsync(undefined);
									// this sucks but oh well :)
									queryClient.setQueryData(['auth.me'], null);
								}}
								disabled={logout.isLoading}
							>
								Logout
							</Button>
						) : (
							<LoginButton onLogin={user.refetch} />
						)
					) : (
						'Loading...'
					)}
				</div>
				<hr className="mb-4 mt-2 flex  w-full border-app-line" />
				{user.data ? (
					<>Loggged in as email {user.data.email}</>
				) : (
					"Login to Spacedrive bc it's cool idk"
				)}
			</div>
		</Card>
	);
}
