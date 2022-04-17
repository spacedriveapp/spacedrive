import React, { useEffect } from 'react';
import { FileList, useExplorerState } from '../components/file/FileList';
import { TopBar } from '../components/layout/TopBar';
import { useParams, useSearchParams } from 'react-router-dom';
import { useBridgeQuery } from '@sd/client';
import { Inspector } from '../components/file/Inspector';

export const ExplorerScreen: React.FC<{}> = () => {
  let [searchParams] = useSearchParams();
  let path = searchParams.get('path') || '';

  let { id } = useParams();
  let location_id = Number(id);

  let [limit, setLimit] = React.useState(100);

  useEffect(() => {
    console.log({ location_id, path, limit });
  }, [location_id, path]);

  const { selectedRowIndex } = useExplorerState();

  const { data: currentDir } = useBridgeQuery(
    'LibGetExplorerDir',
    { location_id: location_id!, path, limit },
    { enabled: !!location_id }
  );

  return (
    <div className="flex flex-col w-full h-full">
      <TopBar />
      <div className="relative flex flex-row w-full ">
        <FileList location_id={location_id} path={path} limit={limit} />
        {currentDir?.contents && (
          <Inspector
            selectedFile={currentDir.contents[selectedRowIndex]}
            locationId={location_id}
          />
        )}
      </div>
    </div>
  );
};
