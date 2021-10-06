import React from 'react';
import { DefaultProps } from '../primative/types';

interface FileRowProps extends DefaultProps {}

export const FileRow: React.FC<FileRowProps> = (props) => {
  return (
    <div className="max-w py-2 px-4 rounded-md bg-gray-50 dark:bg-gray-800">
      <span className="text-white text-sm">Filename.mp4</span>
    </div>
  );
};
