import React, { useState } from 'react';
import { Switch } from '@headlessui/react';
import clsx from 'clsx';

export const Toggle = (
  props: { initialState: boolean; size: 'sm' | 'md' } = { initialState: false, size: 'sm' }
) => {
  const [enabled, setEnabled] = useState(props.initialState || false);

  return (
    <Switch
      checked={enabled}
      onChange={setEnabled}
      className={clsx(
        'transition relative flex-shrink-0 inline-flex items-center h-6 w-11 rounded-full bg-gray-200 dark:bg-gray-550',
        {
          'bg-primary-500 dark:bg-primary-500': enabled,
          'h-6 w-11': props.size === 'sm',
          'h-8 w-[55px]': props.size === 'md'
        }
      )}
    >
      <span
        className={clsx(
          'transition inline-block w-4 h-4 transform bg-white rounded-full',
          enabled ? 'translate-x-6' : 'translate-x-1',
          {
            'w-4 h-4': props.size === 'sm',
            'h-6 w-6': props.size === 'md',
            'translate-x-7': props.size === 'md' && enabled
          }
        )}
      />
    </Switch>
  );
};
