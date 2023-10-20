import { proxy, useSnapshot } from 'valtio';

const windowState = proxy({ isFullScreen: false });

export const useWindowState = () => useSnapshot(windowState);

export const getWindowState = () => windowState;
