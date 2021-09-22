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
        <div className="flex flex-wrap w-full py-2 space-x-2">
          <Button>Button</Button>
          <Button variant="solid">Button</Button>
          <Button variant="light">Button</Button>
          <Button variant="ghost">Button</Button>
          <Button variant="link">Button</Button>
        </div>
        <div className="max-w">
          <Input />
        </div>
        <div className="flex flex-wrap w-full py-2 space-x-2">
          <Button
            variant="solid"
            color="primary"
            leftIcon={<Icon as={CookingPot} label="gift" className="w-4 h-4 mr-1" />}
          >
            Button
          </Button>
          <Button
            color="primary"
            rightIcon={<Icon as={CookingPot} label="gift" className="w-4 h-4 ml-1" />}
          >
            Button
          </Button>
        </div>
        <div className="flex flex-wrap w-full py-2 space-x-4">
          <Switch size="sm" />
          <Switch size="md" />
          <Switch size="lg" />
          <Switch size="xl" />
        </div>
      </div>
    </VechaiProvider>
  );
}
