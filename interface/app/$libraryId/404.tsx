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
				<p className="m-3 text-sm font-semibold uppercase text-ink-faint">Error: 404</p>
				<h1 className="text-4xl font-bold">There's nothing here.</h1>
				<p className="mt-2 text-sm text-ink-dull">
					Its likely that this page has not been built yet, if so we're on it!
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
