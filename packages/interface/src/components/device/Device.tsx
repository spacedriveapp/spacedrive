import {
  Desktop,
  DeviceMobileCamera,
  DotsSixVertical,
  Laptop,
  Phone,
  PhoneX
} from 'phosphor-react';
import React, { useState } from 'react';
import FileItem from '../file/FileItem';
import { Button } from '@sd/ui';
import ProgressBar from '../primitive/ProgressBar';
import { CogIcon } from '@heroicons/react/solid';
import { KeyIcon } from '@heroicons/react/outline';
import LoadingIcons, { Rings } from 'react-loading-icons';

export interface DeviceProps {
  name: string;
  size: string;
  type: 'laptop' | 'desktop' | 'phone';
  locations: { name: string }[];
  runningJob?: { amount: number; task: string };
}

export function Device(props: DeviceProps) {
  const [selectedFile, setSelectedFile] = useState<null | string>(null);

  function handleSelect(key: string) {
    if (selectedFile === key) setSelectedFile(null);
    else setSelectedFile(key);
  }
  return (
    <div className="w-full bg-gray-600 border rounded-md border-gray-550 ">
      <div className="flex flex-row items-center px-4 pt-2 pb-2">
        <DotsSixVertical weight="bold" className="mr-3 opacity-30" />
        {props.type === 'phone' && <DeviceMobileCamera weight="fill" size={20} className="mr-2" />}
        {props.type === 'laptop' && <Laptop weight="fill" size={20} className="mr-2" />}
        {props.type === 'desktop' && <Desktop weight="fill" size={20} className="mr-2" />}
        <h3 className="font-semibold text-md">{props.name}</h3>
        <div className="flex flex-row space-x-1.5 mt-0.5">
          <span className="font-semibold  h-[19px] ml-3 py-0.5 px-1.5 text-[10px] rounded-md text-gray-400 bg-gray-550">
            Primary
          </span>
          <span className="font-semibold  h-[19px] py-0.5 px-1.5 text-[10px]  rounded-md text-gray-400 bg-gray-550">
            {props.size}
          </span>
        </div>
        <div className="flex flex-grow" />
        {props.runningJob && (
          <div className="flex flex-row ml-5 bg-opacity-50 border border-gray-500 rounded bg-gray-550">
            <Rings
              stroke="#2599FF"
              strokeOpacity={4}
              strokeWidth={10}
              speed={0.5}
              className="ml-0.5 mt-0.5 -mr-1 w-7 h-7"
            />
            <div className="flex flex-col p-1.5">
              <span className="mb-[3px] -mt-0.5 truncate text-gray-450 text-tiny">
                {props.runningJob.task}...
              </span>
              <ProgressBar value={props.runningJob?.amount} total={100} />
            </div>
          </div>
        )}
        <div className="flex flex-row ml-3 space-x-1">
          <Button className="!p-1 ">
            <KeyIcon className="w-5 h-5" />
          </Button>
          <Button className="!p-1 ">
            <CogIcon className="w-5 h-5" />
          </Button>
        </div>
      </div>
      <hr className="border-gray-700" />
      <hr className="border-gray-550" />
      <div className="px-4 pb-3 mt-3">
        {props.locations.map((location) => (
          <FileItem
            selected={selectedFile == location.name}
            onClick={() => handleSelect(location.name)}
            fileName={location.name}
            folder
          />
        ))}
      </div>
    </div>
  );
}
