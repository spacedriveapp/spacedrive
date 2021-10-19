import React, { useEffect, useState } from 'react';
import { FileList } from '../components/file/FileList';
import { emit, listen } from '@tauri-apps/api/event';
import { invoke } from '@tauri-apps/api';
import { IFile } from '../types';
import { useExplorerStore } from '../store/explorer';
import { Inspector } from '../components/file/Inspector';

export interface DirectoryResponse {
  directory: IFile;
  contents: IFile[];
}

export const ExplorerScreen: React.FC<{}> = () => {
  const [activeDirHash, collectDir] = useExplorerStore((state) => [
    state.activeDirHash,
    state.collectDir
  ]);

  useEffect(() => {
    invoke<DirectoryResponse>('get_files', { path: '/Users/jamie/Downloads' }).then((res) => {
      console.log({ res });
      collectDir(res.directory, res.contents);
    });
  }, []);

  if (!activeDirHash) return <></>;

  return (
    <div className="relative w-full flex flex-row bg-white dark:bg-gray-900">
      <FileList />
      <Inspector />
    </div>
  );
};
