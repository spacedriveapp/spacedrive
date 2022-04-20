import React from 'react';
import { InputContainer } from '../../components/primitive/InputContainer';
import { Button } from '@sd/ui';

export default function SecuritySettings() {
  return (
    <div className="space-y-4">
      <InputContainer
        title="Something about a vault"
        description="Local cache storage for media previews and thumbnails."
      >
        <div className="flex flex-row">
          <Button variant="primary">Enable Vault</Button>
          {/*<Input className="flex-grow" value="jeff" placeholder="/users/jamie/Desktop" />*/}
        </div>
      </InputContainer>
    </div>
  );
}
