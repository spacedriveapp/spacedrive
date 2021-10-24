import React from 'react';
import { useAppState } from './app';
import { useExplorerStore } from './explorer';

export function DebugGlobalStore() {
  useAppState();
  useExplorerStore();
  return <></>;
}
