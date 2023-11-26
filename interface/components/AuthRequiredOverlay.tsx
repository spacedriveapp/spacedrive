import { auth } from '@sd/client';
import { Loader } from '@sd/ui';
import { LoginButton } from '~/components/LoginButton';

export function AuthRequiredOverlay() {
	const authState = auth.useStateSnapshot();

	if (authState.status !== 'loggedIn')
		return (
			<div className="absolute inset-0 z-50 flex items-center justify-center rounded-md bg-app/75 backdrop-blur-sm">
				{authState.status === 'loading' || authState.status === 'loggingIn' ? (
					<Loader />
				) : (
					<LoginButton />
				)}
			</div>
		);

	return null;
}
