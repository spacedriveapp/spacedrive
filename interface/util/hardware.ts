import * as Icons from '@sd/assets/icons';
import { getIcon } from '@sd/assets/util';
import { HardwareModel } from '@sd/client';

export function hardwareModelToIcon(hardwareModel: HardwareModel) {
	switch (hardwareModel) {
		case 'MacBookPro':
			return 'Laptop';
		case 'MacStudio':
			return 'SilverBox';
		default:
			return 'Laptop';
	}
}
