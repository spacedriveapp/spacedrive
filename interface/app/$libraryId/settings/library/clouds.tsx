import { useLocale } from '~/hooks';

import { Heading } from '../Layout';

export const Component = () => {
	const { t } = useLocale();
	return (
		<>
			<Heading title={t('Clouds')} description={t('clouds_description')} />
		</>
	);
};
