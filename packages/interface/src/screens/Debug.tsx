import { useBridgeCommand, useBridgeQuery } from '@sd/client';
import { Button } from '@sd/ui';
import React from 'react';
import ReactJson from 'react-json-view';
import FileItem from '../components/file/FileItem';
import CodeBlock from '../components/primitive/Codeblock';
import { Tag } from '../components/primitive/Tag';

export const DebugScreen: React.FC<{}> = (props) => {
  const { data: client } = useBridgeQuery('ClientGetState');
  const { data: jobs } = useBridgeQuery('JobGetRunning');
  const { data: jobHistory } = useBridgeQuery('JobGetHistory');
  // const { mutate: purgeDB } = useBridgeCommand('PurgeDatabase', {
  //   onMutate: () => {
  //     alert('Database purged');
  //   }
  // });
  const { mutate: identifyFiles } = useBridgeCommand('IdentifyUniqueFiles');
  return (
    <div className="flex flex-col w-full h-screen p-5 custom-scrollbar page-scroll">
      <div className="flex flex-col space-y-5 pb-7">
        <h1 className="text-lg font-bold ">Developer Debugger</h1>
        <div className="flex flex-row pb-4 space-x-2">
          <Button className="w-40" variant="gray" size="sm" onClick={() => {}}>
            Open data folder
          </Button>

          <Button
            className="w-40"
            variant="gray"
            size="sm"
            onClick={() => identifyFiles(undefined)}
          >
            Identify unique files
          </Button>
        </div>
        <h1 className="text-sm font-bold ">Running Jobs</h1>
        <CodeBlock src={{ ...jobs }} />
        <h1 className="text-sm font-bold ">Job History</h1>
        <CodeBlock src={{ ...jobHistory }} />
        <h1 className="text-sm font-bold ">Client State</h1>
        <CodeBlock src={{ ...client }} />
      </div>
    </div>
  );
};
