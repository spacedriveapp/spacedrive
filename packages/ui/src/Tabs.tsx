import * as TabsPrimitive from '@radix-ui/react-tabs';

import { tw } from './utils';

export const Root = tw(TabsPrimitive.Root)`
  flex flex-col
`;

export const Content = tw(TabsPrimitive.TabsContent)``;

export const List = tw(TabsPrimitive.TabsList)`
  flex flex-row p-2 items-center space-x-1 border-b border-app-line/70
`;

export const Trigger = tw(TabsPrimitive.TabsTrigger)`
  px-1.5 py-0.5 rounded text-sm font-medium radix-state-active:bg-accent radix-state-active:text-white
`;
