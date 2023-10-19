import { proxy, useSnapshot } from 'valtio';

const windowState = proxy({ isMaximized: false });

export const useWindowState = () => useSnapshot(windowState);

export const getWindowState = () => windowState;
