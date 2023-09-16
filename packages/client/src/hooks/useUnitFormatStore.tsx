import { useSnapshot } from 'valtio';

import { valtioPersist } from '../lib';

export type CoordinatesFormat = 'dms' | 'dd';
export type DistanceFormat = 'km' | 'miles';
export type TemperatureFormat = 'celsius' | 'fahrenheit';

interface Schema {
	coordinatesFormat: CoordinatesFormat;
	distanceFormat: DistanceFormat;
	temperatureFormat: TemperatureFormat;
}

const unitFormatStore = valtioPersist<Schema>('sd-display-units', {
	// these are the defaults as 99% of users would want to see them this way
	// if the `en-US` locale is detected during onboarding, the distance/temp are changed to freedom units
	coordinatesFormat: 'dms',
	distanceFormat: 'km',
	temperatureFormat: 'celsius'
} satisfies Schema);

export function useUnitFormatStore() {
	return useSnapshot(unitFormatStore);
}

export function getUnitFormatStore() {
	return unitFormatStore;
}
