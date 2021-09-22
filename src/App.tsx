import React from 'react';
import { Button, colors, ColorScheme, extendTheme, Input } from '@vechaiui/react';
import { VechaiProvider } from '@vechaiui/react';

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
      <div className="p-8">
        <div className="flex flex-wrap w-full p-8 space-x-2">
          <Button>Button</Button>
          <Button variant="solid">Button</Button>
          <Button variant="light">Button</Button>
          <Button variant="ghost">Button</Button>
          <Button variant="link">Button</Button>
        </div>
        <Input />
      </div>
    </VechaiProvider>
  );
}
