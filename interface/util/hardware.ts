// import * as Icons from '@sd/assets/icons';
// import { getIcon } from '@sd/assets/util';
import { HardwareModel } from '@sd/client';

export function hardwareModelToIcon(hardwareModel: HardwareModel) {
	switch (hardwareModel) {
		case 'MacBookPro':
			return 'Laptop';
		case 'MacStudio':
			return 'SilverBox';
		case 'IPhone':
			return 'Mobile';
		case 'Android':
			return 'MobileAndroid';
		case 'MacMini':
			return 'MiniSilverBox';
		case 'Other':
			return 'PC';
		default:
			return 'Laptop';
	}
}
