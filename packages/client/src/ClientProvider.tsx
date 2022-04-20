import React from 'react';
import { QueryClientProvider } from 'react-query';

// The ClientProvider injects the React-query context into the "context store" of the current package. This is needed due to the fact the repository is a monorepo.
// This is a pretty hacky solution and a better solution should probably be found to replace it.
export function ClientProvider({ children }: any) {
  return (
    // @ts-ignore: This exists to add the QueryClientProvider to the current subpackage '@sd/client'. The ReactQueryClient is fetched from the window object (which is set in the parent application).
    <QueryClientProvider client={window.ReactQueryClient}>{children}</QueryClientProvider>
  );
}
