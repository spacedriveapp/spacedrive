import React from 'react';
import { useAppState } from './store/global';
import { useExplorerStore } from './store/explorer';

export function DebugGlobalStore() {
  useAppState();
  useExplorerStore();
  return <></>;
}
