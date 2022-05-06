import React from 'react';
import { InputContainer } from '../../components/primitive/InputContainer';

export default function LocationSettings() {
  // const locations = useBridgeQuery("SysGetLocation")

  return (
    <div className="flex flex-col flex-grow max-w-4xl space-y-4">
      {/*<Button size="sm">Add Location</Button>*/}
      <InputContainer
        title="Something about a vault"
        description="Local cache storage for media previews and thumbnails."
      >
        <div className="flex flex-row space-x-2"></div>
      </InputContainer>
    </div>
  );
}
