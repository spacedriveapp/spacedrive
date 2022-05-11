import React from 'react';
import { InputContainer } from '../../components/primitive/InputContainer';
import { Toggle } from '../../components/primitive';

type LibrarySecurity = 'public' | 'password' | 'vault';

export default function LibrarySettings() {
  // const locations = useBridgeQuery("SysGetLocation")
  const [encryptOnCloud, setEncryptOnCloud] = React.useState<boolean>(false);

  return (
    <div className="flex flex-col flex-grow max-w-4xl space-y-4">
      {/*<Button size="sm">Add Location</Button>*/}
      <div className="mt-3 mb-3">
        <h1 className="text-2xl font-bold">Library database</h1>
        <p className="mt-1 text-sm text-gray-400">
          The database contains all library data and file metadata.
        </p>
      </div>
      <InputContainer
        mini
        title="Encrypt on cloud"
        description="Enable if library contains sensitive data and should not be synced to the cloud without full encryption."
      >
        <div className="flex items-center h-full">
          <Toggle value={encryptOnCloud} onChange={setEncryptOnCloud} size={'sm'} />
        </div>
      </InputContainer>
    </div>
  );
}
