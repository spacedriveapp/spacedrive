import { createMutable } from 'solid-js/store';

import { createPersistedMutable, useSolidStore } from '../solid';

export type CoordinatesFormat = 'dms' | 'dd';
export type DistanceFormat = 'km' | 'miles';
export type TemperatureFormat = 'celsius' | 'fahrenheit';

export const unitFormatStore = createPersistedMutable(
	'sd-display-units',
	createMutable({
		// these are the defaults as 99% of users would want to see them this way
		// if the `en-US` locale is detected during onboarding, the distance/temp are changed to freedom units
		coordinatesFormat: 'dms' as CoordinatesFormat,
		distanceFormat: 'km' as DistanceFormat,
		temperatureFormat: 'celsius' as TemperatureFormat
	})
);

export function useUnitFormatStore() {
	return useSolidStore(unitFormatStore);
}
