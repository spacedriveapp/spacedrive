import React, { useRef, useState } from 'react';
import { Button, colors, ColorScheme, extendTheme, Icon, Input, Switch } from '@vechaiui/react';
import { VechaiProvider } from '@vechaiui/react';
import { CookingPot } from 'phosphor-react';
import { invoke } from '@tauri-apps/api';

export const pale: ColorScheme = {
  id: 'pale',
  type: 'dark',
  colors: {
    bg: {
      base: colors.blueGray['800'],
      fill: colors.blueGray['900']
    },
    text: {
      foreground: colors.blueGray['100'],
      muted: colors.blueGray['300']
    },
    primary: colors.violet,
    neutral: colors.blueGray
  }
};

const theme = extendTheme({
  cursor: 'pointer',
  colorSchemes: {
    pale
  }
});

export default function App() {
  const fileUploader = useRef<HTMLInputElement | null>(null);
  const [fileInputVal, setFileInputVal] = useState('/Users/jamie/Downloads/lol.mkv');

  function changeHandler(e: any) {
    console.log(e);
  }

  return (
    <VechaiProvider theme={theme} colorScheme="pale">
      <div data-tauri-drag-region className="max-w h-4"></div>
      <div className="p-2">
        <div className="flex flex-wrap w-full space-x-2">
          <Input
            className="mb-2"
            value={fileInputVal}
            onChange={(e) => setFileInputVal(e.target.value)}
          />
          <input
            ref={fileUploader}
            type="file"
            id="file"
            className="hidden"
            onChange={changeHandler}
          />
          <Button
            variant="solid"
            color="primary"
            onClick={() => {
              invoke('read_file_command', {
                path: fileInputVal
              }).then(console.log);
            }}
          >
            Load File
          </Button>
          <Button
            variant="solid"
            color="primary"
            onClick={() => {
              invoke('generate_buffer_checksum', {
                path: fileInputVal
              }).then(console.log);
            }}
          >
            Generate Buffer Checksum
          </Button>
          <Button variant="solid" color="primary">
            Close
          </Button>
        </div>
      </div>
    </VechaiProvider>
  );
}
