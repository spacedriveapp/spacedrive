import React, { useEffect } from 'react';
// import { FileList } from '../components/file/FileList';
import { invoke } from '@tauri-apps/api';

import { Inspector } from '../components/file/Inspector';

export const ExplorerScreen: React.FC<{}> = () => {
  // let { slug } = useParams();

  return (
    <div className="relative flex flex-row w-full bg-white dark:bg-gray-900">
      {/* <FileList /> */}
      {/* <Inspector /> */}
    </div>
  );
};
