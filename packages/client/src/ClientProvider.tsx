import React from 'react';
import { QueryClientProvider, QueryClientProviderProps } from 'react-query';

export interface ClientProviderProps extends Omit<QueryClientProviderProps, 'client'> {
	children?: React.ReactNode;
}

// The ClientProvider injects the React-query context into the "context store" of the current package. This is needed due to the fact the repository is a monorepo.
// This is a pretty hacky solution and a better solution should probably be found to replace it.
export const ClientProvider: React.FC<ClientProviderProps> = ({ children, ...props }) => {
	return (
		// This exists to add the QueryClientProvider to the current subpackage '@sd/client'.
		// The ReactQueryClient is fetched from the window object (which is set in the parent application).
		// @ts-expect-error: react-query depends on the react@^17 FC type which includes children -- once v4.0 is released this will not be necessary
		<QueryClientProvider {...props} client={window.ReactQueryClient}>
			{children}
		</QueryClientProvider>
	);
};
