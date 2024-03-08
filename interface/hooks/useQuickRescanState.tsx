import { proxy, useSnapshot } from 'valtio';

const quickRescanState = proxy({ lastRun: Date.now() - 200 });

export const useQuickRescanState = () => useSnapshot(quickRescanState);

export const getQuickRescanState = () => quickRescanState;
