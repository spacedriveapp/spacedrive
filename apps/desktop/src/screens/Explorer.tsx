import React, { useEffect } from 'react';
import { FileList, useExplorerState } from '../components/file/FileList';
import { invoke } from '@tauri-apps/api';
import { TopBar } from '../components/layout/TopBar';
import { useSearchParams } from 'react-router-dom';
import { useBridgeQuery } from '@sd/client';
import { Inspector } from '../components/file/Inspector';

export const ExplorerScreen: React.FC<{}> = () => {
  // let { slug } = useParams();
  let [searchParams] = useSearchParams();
  let path = searchParams.get('path');

  let [locationId, _setLocationId] = React.useState(1);
  let [limit, setLimit] = React.useState(100);

  const { data: currentDir } = useBridgeQuery(
    'LibGetExplorerDir',
    {
      location_id: locationId,
      path: path || '/',
      limit
    },
    {
      enabled: !!path
    }
  );
  const { selectedRowIndex } = useExplorerState();

  return (
    <div className="flex flex-col w-full h-full">
      <TopBar />
      <div className="relative flex flex-row w-full ">
        <FileList location_id={1} path={path ?? ''} limit={limit} />
        <Inspector selectedFile={currentDir?.contents[selectedRowIndex]} locationId={locationId} />
      </div>
    </div>
  );
};
