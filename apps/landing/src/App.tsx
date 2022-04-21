import { Button } from '@sd/ui';
import { Bubbles } from './Bubbles';
import './index.css';

import { ReactComponent as GithubLogo } from './assets/github.svg';
import { ReactComponent as AppleLogo } from './assets/apple.svg';

function App() {
  return (
    <div>
      <div className="container z-10 flex flex-col items-center px-4 mx-auto text-white bg-black">
        <img src="app-logo.svg" className="z-50 w-40 pt-20" />
        <h1 className="mt-10 text-6xl font-black">The file explorer from the future</h1>
        <p className="max-w-5xl mt-1 mb-6 text-lg text-center text-gray-450">
          Spacedrive is the first file explorer that puts the full power of the cloud in your hands.
        </p>
        <div className="flex flex-row space-x-4">
          {/* <Button className="px-2">
          <WindowsLogo className="" fill="white" />
        </Button> */}
          <Button variant="primary">
            <AppleLogo className="inline -mt-[3px] mr-1.5" fill="white" />
            Download
          </Button>
          <Button variant="gray">
            <GithubLogo className="inline -mt-[3px] mr-1.5" fill="white" />
            Star on GitHub
          </Button>
        </div>
        <p className="text-xs text-center text-gray-500 mt-7">
          Available on macOS (Intel & Apple Silicon), Windows and Linux.
          <br />
          Coming soon to iOS & Android.
        </p>

        <iframe
          className="z-50 mt-10 border rounded-lg shadow-2xl bg-gray-850 border-gray-550"
          width={1200}
          height={600}
          src="http://localhost:8002?library_id=9068c6ec-cf90-451b-bb30-4174781e7bc6"
        />
        <div className="grid grid-cols-2 my-32">
          <div className="p-10">
            <h1 className="text-4xl font-black">See the bigger picture</h1>
            <p className="mt-5 text-xl text-gray-450">
              Using content addressable storage in a virtual distributed filesystem, Spacedrive
              securely combines the storage capacity and processing power of your devices into one.
            </p>
          </div>
          <div className="p-10 "></div>
        </div>
        <div className="h-[500px]" />
      </div>
      <Bubbles />
    </div>
  );
}

export default App;
