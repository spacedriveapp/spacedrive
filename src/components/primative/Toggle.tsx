import React from 'react';
import { useState } from 'react';
import { Switch } from '@headlessui/react';
import clsx from 'clsx';

export const Toggle = (props: { initialState: boolean }) => {
  const [enabled, setEnabled] = useState(props.initialState || false);

  return (
    <Switch
      checked={enabled}
      onChange={setEnabled}
      className={clsx(
        'relative inline-flex items-center h-6 rounded-full w-11 bg-gray-200 dark:bg-gray-700',
        {
          'bg-primary-500 dark:bg-primary-500': enabled
        }
      )}
    >
      <span
        className={`${
          enabled ? 'translate-x-6' : 'translate-x-1'
        } inline-block w-4 h-4 transform bg-white rounded-full`}
      />
    </Switch>
  );
};
