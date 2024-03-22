import { Slider } from '@sd/ui';
import { useLocale } from '~/hooks';

import { Subheading } from '..';
import { useExplorerContext } from '../../Context';
import { LIST_VIEW_TEXT_SIZES } from '../../View/ListView/useTable';
import { getSizeOptions } from './util';

const sizes = Object.keys(LIST_VIEW_TEXT_SIZES) as (keyof typeof LIST_VIEW_TEXT_SIZES)[];
const options = getSizeOptions(sizes);

export const TextSize = () => {
	const { t } = useLocale();

	const explorer = useExplorerContext();
	const settings = explorer.useSettingsSnapshot();

	return (
		<div>
			<Subheading>{t('text_size')}</Subheading>
			<Slider
				step={1}
				max={sizes.length - 1}
				defaultValue={[options[settings.listViewTextSize]]}
				onValueChange={([value]) => {
					const size = value !== undefined && sizes[value];
					if (size) explorer.settingsStore.listViewTextSize = size;
				}}
			/>
		</div>
	);
};
