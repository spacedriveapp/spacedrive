import clsx from 'clsx';
import React from 'react';
import { DefaultProps } from '../primitive/types';

interface Props extends DefaultProps {
  fileName: string;
  iconName?: string;
  format?: string;
  folder?: boolean;
  selected?: boolean;
  onClick?: () => void;
}

export default function FileItem(props: Props) {
  return (
    <div onClick={props.onClick} className="inline-block w-[100px]  mb-3">
      <div
        className={clsx(
          'inline-block border-2 border-transparent rounded-lg text-center w-[100px] h-[100px] ',
          { ' bg-gray-550': props.selected }
        )}
      >
        {/* <div className="w-[65px] border border-gray-600 m-auto rounded-md h-[80px] bg-gray-650 relative shadow-md "> */}
        {props.folder ? (
          <div className="w-full h-full">
            <img className="bottom-0 p-3 pt-[19px] margin-auto" src="svg/folder.svg" />
          </div>
        ) : (
          <div className="w-[64px] mt-1.5 m-auto rounded-lg h-[80px] relative">
            <svg
              className="absolute top-0 left-0 fill-gray-750"
              width="65"
              height="81"
              viewBox="0 0 65 81"
            >
              <path d="M0 8C0 3.58172 3.58172 0 8 0H39.6863C41.808 0 43.8429 0.842855 45.3431 2.34315L53.5 10.5L62.6569 19.6569C64.1571 21.1571 65 23.192 65 25.3137V73C65 77.4183 61.4183 81 57 81H8C3.58172 81 0 77.4183 0 73V8Z" />
            </svg>
            <svg
              width="22"
              height="22"
              className="absolute -right-[1px] z-10 fill-gray-500"
              viewBox="0 0 41 41"
            >
              <path d="M41.4116 40.5577H11.234C5.02962 40.5577 0 35.5281 0 29.3238V0L41.4116 40.5577Z" />
            </svg>
            <div className="absolute flex flex-col items-center justify-center w-full h-full">
              <img
                className="mt-2 margin-auto"
                width={40}
                height={40}
                src={`icons/${props.iconName}.svg`}
              />
              <span className="mt-1 text-xs font-bold text-center uppercase cursor-default text-gray-450">
                {props.format}
              </span>
            </div>
          </div>
        )}
      </div>
      <div className="flex justify-center">
        <span
          className={clsx(
            'px-1.5 py-[1px] rounded-md text-sm font-medium text-gray-300 cursor-default',
            {
              'bg-primary text-white': props.selected
            }
          )}
        >
          {props.fileName}
        </span>
      </div>
    </div>
  );
}
