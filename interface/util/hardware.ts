import * as Icons from '@sd/assets/icons';
import { getIcon } from '@sd/assets/util';
import { HardwareModel } from '@sd/client';

export function hardwareModelToIcon(hardwareModel: HardwareModel) {
	switch (hardwareModel) {
		case 'MacBookPro':
			return getIcon('Laptop');
		case 'MacStudio':
			return getIcon('SilverBox');
		default:
			return getIcon('Laptop');
	}
}
