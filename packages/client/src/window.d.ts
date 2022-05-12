import type { QueryClient } from 'react-query/types';

declare global {
  interface Window {
    ReactQueryClient: QueryClient;
  }
}
