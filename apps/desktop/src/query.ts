import { QueryClient } from '@tanstack/react-query';

export const queryClient = new QueryClient({
	defaultOptions: {
		queries: {
			networkMode: 'always'
		},
		mutations: {
			networkMode: 'always'
		}
	}
});
