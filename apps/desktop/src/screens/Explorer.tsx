import React, { useEffect, useState } from 'react';
import { FileList } from '../components/file/FileList';
import { emit, listen } from '@tauri-apps/api/event';
import { invoke } from '@tauri-apps/api';
import { IFile } from '../types';
import { useExplorerStore } from '../store/explorer';
import { Inspector } from '../components/file/Inspector';
import {useParams} from "react-router-dom";

export interface DirectoryResponse {
  directory: IFile;
  contents: IFile[];
}

export const ExplorerScreen: React.FC<{}> = () => {

  // let { slug } = useParams();

  const [currentDir, tempWatchDir] = useExplorerStore((state) => [
    state.currentDir,
    state.tempWatchDir
  ]);

  useEffect(() => {
    invoke<DirectoryResponse>('get_files', { path: tempWatchDir }).then((res) => {
      console.log({ res });
      useExplorerStore.getState().ingestDir(res.directory, res.contents);
    });
  }, []);

  if (currentDir === null) return <></>;

  return (
    <div className="relative flex flex-row w-full bg-white dark:bg-gray-900">
      <FileList />
      <Inspector />
    </div>
  );
};
