import { useSnapshot } from 'valtio';

import { valtioPersist } from '../lib';

export type CoordinatesFormat = 'dms' | 'dd';
export type DistanceFormat = 'km' | 'miles';
export type TemperatureFormat = 'celsius' | 'fahrenheit';

const formatStore = valtioPersist('sd-display-units', {
	// these are the defaults as 99% of users would want to see them this way
	// if the `en-US` locale is detected during onboarding, the distance/temp are changed to freedom units
	coordinatesFormat: 'dms' satisfies CoordinatesFormat,
	distanceFormat: 'km' satisfies DistanceFormat,
	temperatureFormat: 'celsius' satisfies TemperatureFormat
});

export function useFormatStore() {
	return useSnapshot(formatStore);
}

export function getFormatStore() {
	return formatStore;
}
