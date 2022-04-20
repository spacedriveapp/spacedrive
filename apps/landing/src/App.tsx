import { useState } from 'react';
import { Button } from '@sd/ui';
import { Bubbles } from './Bubbles';

function App() {
  return (
    <div className="flex flex-col items-center h-screen text-white bg-black">
      <img src="app-logo.svg" className="z-50 w-40 pt-20" />
      <h1 className="mt-10 text-6xl font-black">The file explorer from the future</h1>
      <p className="mt-1 mb-10 text-lg text-gray-450">
        Spacedrive is the first file explorer that puts the full power of the cloud in your hands.
      </p>
      <Button variant="primary" className="mb-10">
        Download
      </Button>

      <iframe
        className="z-50 border rounded-lg shadow-2xl border-gray-550"
        width={1200}
        height={600}
        src="http://localhost:8002?library_id=9068c6ec-cf90-451b-bb30-4174781e7bc6"
      />
      <Bubbles />
    </div>
  );
}

export default App;
