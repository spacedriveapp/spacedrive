import { createContext } from 'react';

export const LocationContext = createContext<{
	location_id: number;
	data_path: string;
}>({
	location_id: 1,
	data_path: ''
});
