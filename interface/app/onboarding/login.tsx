import { AppLogo } from '@sd/assets/images';
import { useNavigate } from 'react-router';
import { auth, useBridgeQuery } from '@sd/client';
import { Button, ButtonLink, Loader } from '@sd/ui';
import { LoginButton } from '~/components/LoginButton';
import { useLocale } from '~/hooks';

import { OnboardingContainer } from './components';

export default function OnboardingLogin() {
	const { t } = useLocale();

	const authState = auth.useStateSnapshot();
	const navigate = useNavigate();

	// const me = useBridgeQuery(['auth.me'], { retry: false });

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
							Logged in as <b> TODO </b>
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
							{t('continue')}
						</ButtonLink>

						<div className="space-x-2 text-center text-sm">
							<span>{t('not_you')}</span>
							<Button
								onClick={auth.logout}
								variant="bare"
								size="md"
								className="border-none !p-0 font-normal text-accent-deep hover:underline"
							>
								{t('log_out')}
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
						<h1 className="text-lg text-ink">Log in to Spacedrive</h1>
					</div>

					<div className="mt-10 flex w-[250px] flex-col gap-3">
						<LoginButton
							onLogin={() => navigate('../new-library', { replace: true })}
							size="md"
						>
							{t('log_in_with_browser')}
						</LoginButton>

						<div className="space-x-2 text-center text-sm">
							<span>{t('want_to_do_this_later')}</span>
							<ButtonLink
								to="../new-library"
								variant="bare"
								size="md"
								className="border-none !p-0 font-normal text-accent-deep hover:underline"
								replace
							>
								{t('skip_login')}
							</ButtonLink>
						</div>
					</div>
				</>
			)}
		</OnboardingContainer>
	);
}
