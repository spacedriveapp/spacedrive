import { useNavigate } from 'react-router';
import { Button } from '@sd/ui';
import { useLocale } from '~/hooks';

export const Component = () => {
	const navigate = useNavigate();

	const { t } = useLocale();

	return (
		<div className="w-full bg-app/80">
			<div
				role="alert"
				className="flex size-full flex-col items-center justify-center rounded-lg p-4"
			>
				<p className="m-3 text-sm font-semibold uppercase text-ink-faint">
					Spacedrive is in beta!
				</p>
				<h1 className="text-4xl font-bold">There's nothing here.</h1>
				<p className="mt-2 text-sm text-ink-dull">
					This is most likely a bug, please report it to us on{' '}
					<a href="https://discord.gg/gTaF2Z44f5" className="text-accent hover:underline">
						Discord
					</a>{' '}
					or{' '}
					<a
						href="https://github.com/spacedriveapp/spacedrive/issues"
						className="text-accent hover:underline"
					>
						GitHub
					</a>
					.
				</p>
				<div className="flex flex-row space-x-2">
					<Button variant="outline" className="mt-4" onClick={() => navigate(-1)}>
						← {t('go_back')}
					</Button>
				</div>
			</div>
		</div>
	);
};
