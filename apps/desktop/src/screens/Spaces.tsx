import React from 'react';

export const SpacesScreen: React.FC<{}> = (props) => {
  return (
    <div className="flex flex-col w-full h-full p-5">
      <h1 className="text-xl font-bold ">Spaces</h1>

      <div className="flex flex-wrap p-2 my-3 space-x-2 bg-black rounded">
        <div className="w-10 h-10 rounded bg-gray-950"/>
        <div className="w-10 h-10 bg-gray-900 rounded"/>
        <div className="w-10 h-10 rounded bg-gray-850"/>
        <div className="w-10 h-10 bg-gray-800 rounded"/>
        <div className="w-10 h-10 rounded bg-gray-750"/>
        <div className="w-10 h-10 bg-gray-700 rounded"/>
        <div className="w-10 h-10 rounded bg-gray-650"/>
        <div className="w-10 h-10 bg-gray-600 rounded"/>
        <div className="w-10 h-10 rounded bg-gray-550"/>
        <div className="w-10 h-10 bg-gray-400 rounded"/>
        <div className="w-10 h-10 rounded bg-gray-450"/>
        <div className="w-10 h-10 bg-gray-400 rounded"/>
        <div className="w-10 h-10 rounded bg-gray-350"/>
        <div className="w-10 h-10 bg-gray-300 rounded"/>
        <div className="w-10 h-10 rounded bg-gray-250"/>
        <div className="w-10 h-10 bg-gray-200 rounded"/>
        <div className="w-10 h-10 rounded bg-gray-150"/>
        <div className="w-10 h-10 bg-gray-100 rounded"/>
        <div className="w-10 h-10 rounded bg-gray-50"/>
      </div>
    </div>
  );
};
