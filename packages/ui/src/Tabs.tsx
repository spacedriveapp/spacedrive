import * as TabsPrimitive from '@radix-ui/react-tabs';
import tw from 'tailwind-styled-components';

export const Root = tw(TabsPrimitive.Root)`
  flex flex-col 
`;

export const Content = tw(TabsPrimitive.TabsContent)``;

export const List = tw(TabsPrimitive.TabsList)`
  flex flex-row p-2 items-center space-x-1 border-b border-gray-500/30
`;

export const Trigger = tw(TabsPrimitive.TabsTrigger)`
  text-white px-1.5 py-0.5 rounded text-sm font-medium radix-state-active:bg-primary
`;
