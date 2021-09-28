import React from 'react';
import { Button, colors, ColorScheme, extendTheme, Icon, Input, Switch } from '@vechaiui/react';
import { VechaiProvider } from '@vechaiui/react';
import { CookingPot } from 'phosphor-react';

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
  return (
    <VechaiProvider theme={theme} colorScheme="pale">
      <div className="p-2">
        <div className="max-w h-20"></div>
        <div className="flex flex-wrap w-full py-2 space-x-2">
          <Button variant="solid" color="primary">
            Load File
          </Button>
        </div>
      </div>
    </VechaiProvider>
  );
}
