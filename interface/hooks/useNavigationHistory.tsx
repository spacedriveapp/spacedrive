import { useSnapshot } from 'valtio';
import { proxySet } from 'valtio/utils';

const navigationHistory = proxySet<string>();

export const useNavigationHistory = () => useSnapshot(navigationHistory);

export const getNavigationHistory = () => navigationHistory;
