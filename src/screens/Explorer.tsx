import React, { useEffect, useState } from 'react';
import { FileList } from '../components/file/FileList';
import { emit, listen } from '@tauri-apps/api/event';
import { invoke } from '@tauri-apps/api';
import { FileData } from '../types';

export const ExplorerScreen: React.FC<{}> = () => {
  const [files, setFiles] = useState<FileData[] | null>(null);
  useEffect(() => {
    invoke('get_files').then((res) => {
      setFiles(res as FileData[]);
    });
  }, []);
  console.log({ files });

  if (!files) return <></>;

  return (
    <div className="w-full m-3">
      <FileList files={files} />
    </div>
  );
};
