import { InputContainer } from '../../components/primitive/InputContainer';
import { Button, Input } from '../../components/primitive';
import { invoke } from '@tauri-apps/api';
import React, { useRef } from 'react';
import { useExplorerStore } from '../../store/explorer';
import { useAppState } from '../../store/global';

export default function GeneralSettings() {
  const fileUploader = useRef<HTMLInputElement | null>(null);
  const config = useAppState();

  const [tempWatchDir, setTempWatchDir] = useExplorerStore((state) => [
    state.tempWatchDir,
    state.setTempWatchDir
  ]);
  return (
    <div className="space-y-4">
      <InputContainer
        title="Quick scan directory"
        description="The directory for which this application will perform a detailed scan of the contents and sub directories"
      >
        <div className="flex flex-row">
          <Input
            className="w-3/5"
            value={tempWatchDir}
            onChange={(e) => setTempWatchDir(e.target.value)}
            placeholder="/users/jamie/Desktop"
          />
          <Button
            className="ml-2"
            variant="primary"
            onClick={async () => {
              await invoke('scan_dir', {
                path: tempWatchDir
              });
            }}
          >
            Scan Now
          </Button>
        </div>
      </InputContainer>
      <InputContainer
        title="Media cache directory"
        description="Local cache storage for media previews and thumbnails."
      >
        <div className="flex flex-row">
          <Input
            className="w-3/5"
            value={config.file_type_thumb_dir}
            placeholder="/users/jamie/Desktop"
          />
        </div>
      </InputContainer>
      <InputContainer
        title="Media cache directory"
        description="Local cache storage for media previews and thumbnails."
      >
        <div className="flex flex-row">
          <Input
            className="w-3/5"
            value={config.file_type_thumb_dir}
            placeholder="/users/jamie/Desktop"
          />
        </div>
      </InputContainer>
      <InputContainer
        title="Media cache directory"
        description="Local cache storage for media previews and thumbnails."
      >
        <div className="flex flex-row">
          <Input
            className="w-3/5"
            value={config.file_type_thumb_dir}
            placeholder="/users/jamie/Desktop"
          />
        </div>
      </InputContainer>
      <InputContainer
        title="Media cache directory"
        description="Local cache storage for media previews and thumbnails."
      >
        <div className="flex flex-row">
          <Input
            className="w-3/5"
            value={config.file_type_thumb_dir}
            placeholder="/users/jamie/Desktop"
          />
        </div>
      </InputContainer>
      <InputContainer
        title="Media cache directory"
        description="Local cache storage for media previews and thumbnails."
      >
        <div className="flex flex-row">
          <Input
            className="w-3/5"
            value={config.file_type_thumb_dir}
            placeholder="/users/jamie/Desktop"
          />
        </div>
      </InputContainer>
      <InputContainer
        title="Media cache directory"
        description="Local cache storage for media previews and thumbnails."
      >
        <div className="flex flex-row">
          <Input
            className="w-3/5"
            value={config.file_type_thumb_dir}
            placeholder="/users/jamie/Desktop"
          />
        </div>
      </InputContainer>
    </div>
  );
}
