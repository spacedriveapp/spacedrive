import React from 'react';
import { useAppState } from './store/app';
import { useExplorerStore } from './store/explorer';

export function DebugGlobalStore() {
  useAppState();
  useExplorerStore();
  return <></>;
}
