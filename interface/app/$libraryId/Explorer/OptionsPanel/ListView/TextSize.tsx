import { useMemo } from 'react';
import { Slider } from '@sd/ui';
import { useLocale } from '~/hooks';

import { Subheading } from '..';
import { useExplorerContext } from '../../Context';
import { LIST_VIEW_TEXT_SIZES } from '../../View/ListView/useTable';
import { getSizes } from './util';

const sizes = getSizes(LIST_VIEW_TEXT_SIZES);

export const TextSize = () => {
	const { t } = useLocale();

	const explorer = useExplorerContext();
	const settings = explorer.useSettingsSnapshot();

	const defaultValue = useMemo(
		() => sizes.findIndex((size) => size[0] === settings.listViewTextSize),
		[settings.listViewTextSize]
	);

	return (
		<div>
			<Subheading>{t('text_size')}</Subheading>
			<Slider
				step={1}
				max={sizes.length - 1}
				defaultValue={[defaultValue]}
				onValueChange={([value]) => {
					const size = value !== undefined && sizes[value];
					if (size) explorer.settingsStore.listViewTextSize = size[0];
				}}
			/>
		</div>
	);
};
