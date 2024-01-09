import { useLocale } from '~/hooks';

import { Heading } from '../Layout';

export const Component = () => {
	const { t } = useLocale();

	return (
		<>
			<Heading title={t('contacts')} description={t('contacts_description')} />
		</>
	);
};
