import { AppLogo } from '@sd/assets/images';
import { useNavigate } from 'react-router';
import { auth, useBridgeQuery } from '@sd/client';
import { Button, ButtonLink, Loader } from '@sd/ui';
import { LoginButton } from '~/components/LoginButton';

import { OnboardingContainer } from './Layout';

export default function OnboardingLogin() {
	const authState = auth.useStateSnapshot();
	const navigate = useNavigate();

	const me = useBridgeQuery(['auth.me'], { retry: false });

	return (
		<OnboardingContainer>
			{authState.status === 'loading' ? (
				<Loader />
			) : authState.status === 'loggedIn' ? (
				<>
					<div className="flex flex-col items-center justify-center">
						<img
							src={AppLogo}
							alt="Spacedrive logo"
							width={50}
							height={50}
							draggable={false}
							className="mb-3"
						/>
						<h1 className="text-lg text-ink">
							Logged in as <b>{me.data?.email}</b>
						</h1>
					</div>

					<div className="mt-10 flex w-[250px] flex-col gap-3">
						<ButtonLink
							to="../new-library"
							replace
							variant="accent"
							size="md"
							className="text-center"
						>
							Continue
						</ButtonLink>

						<div className="space-x-2 text-center text-sm">
							<span>Not you?</span>
							<Button
								onClick={auth.logout}
								variant="bare"
								size="md"
								className="border-none !p-0 font-normal text-accent-deep hover:underline"
							>
								Log out
							</Button>
						</div>
					</div>
				</>
			) : (
				<>
					<div className="flex flex-col items-center justify-center">
						<img
							src={AppLogo}
							alt="Spacedrive logo"
							width={50}
							height={50}
							draggable={false}
							className="mb-3"
						/>
						<h1 className="text-lg text-ink">
							Log in to <b>Spacedrive</b>
						</h1>
					</div>

					<div className="mt-10 flex w-[250px] flex-col gap-3">
						<LoginButton
							onLogin={() => navigate('../new-library', { replace: true })}
							size="md"
						>
							Log in with browser
						</LoginButton>

						<div className="space-x-2 text-center text-sm">
							<span>Want to do this later?</span>
							<ButtonLink
								to="../new-library"
								variant="bare"
								size="md"
								className="border-none !p-0 font-normal text-accent-deep hover:underline"
								replace
							>
								Skip login
							</ButtonLink>
						</div>
					</div>
				</>
			)}
		</OnboardingContainer>
	);
}
