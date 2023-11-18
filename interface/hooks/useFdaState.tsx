import { proxy, useSnapshot } from 'valtio';

const fdaState = proxy({ fda: false });

export const useFdaState = () => useSnapshot(fdaState);

export const getFdaState = () => fdaState;
