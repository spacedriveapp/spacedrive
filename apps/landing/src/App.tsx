import { Button } from '@sd/ui';
import { Bubbles } from './Bubbles';
import './index.css';

import { ReactComponent as GithubLogo } from './assets/github.svg';

function App() {
  return (
    <div className="flex flex-col items-center text-white bg-black">
      <img src="app-logo.svg" className="z-50 w-40 pt-20" />
      <h1 className="mt-10 text-6xl font-black">The file explorer from the future</h1>
      <p className="mt-1 mb-10 text-lg text-gray-450">
        Spacedrive is the first file explorer that puts the full power of the cloud in your hands.
      </p>
      <div className="flex flex-row space-x-4">
        <Button variant="primary" className="mb-10">
          Download
        </Button>
        <Button variant="gray" className="mb-10">
          <GithubLogo className="inline -mt-[3px] mr-1.5" fill="white" />
          Star on GitHub
        </Button>
      </div>

      <iframe
        className="z-50 border rounded-lg shadow-2xl border-gray-550"
        width={1200}
        height={600}
        src="http://localhost:8002?library_id=9068c6ec-cf90-451b-bb30-4174781e7bc6"
      />
      <div className="h-[500px]" />
      <Bubbles />
    </div>
  );
}

export default App;
