import { Button } from '@sd/ui';
import { Bubbles } from './Bubbles';
import './index.css';

import { ReactComponent as GithubLogo } from './assets/github.svg';
import { ReactComponent as AppleLogo } from './assets/apple.svg';
import { ReactComponent as AppLogo } from './assets/app-logo.svg';
import NavBar from './components/NavBar';

interface SectionProps {
  orientation: 'left' | 'right';
  heading?: string;
  description?: string;
  children?: React.ReactNode;
}

function Section(props: SectionProps = { orientation: 'left' }) {
  let info = (
    <div className="p-10">
      {props.heading && <h1 className="text-4xl font-black">{props.heading}</h1>}
      {props.description && <p className="mt-5 text-xl text-gray-450">{props.description}</p>}
    </div>
  );
  let children = <div className="p-10 ">{props.children}</div>;
  return (
    <div className="grid grid-cols-2 my-44">
      {props.orientation === 'right' ? (
        <>
          {info}
          {children}
        </>
      ) : (
        <>
          {children}
          {info}
        </>
      )}
    </div>
  );
}

function App() {
  return (
    <div>
      <NavBar />
      <div className="container z-10 flex flex-col items-center px-4 mx-auto text-white bg-black">
        {/* <AppLogo className="z-30 w-40 h-40 mt-32" /> */}
        <h1 className="text-4xl font-black leading-snug text-center md:text-6xl mt-36">
          The file explorer from the future
        </h1>
        <p className="max-w-3xl mt-1 mb-8 text-lg text-center text-gray-450">
          Spacedrive allows you to manage files across all devices, drives and clouds at once.
          <br />
          Designed for creators, hoarders and the painfully disorganized.
        </p>
        <div className="flex flex-row space-x-4">
          {/* <Button className="px-2">
          <WindowsLogo className="" fill="white" />
        </Button> */}
          <Button className="cursor-pointer" variant="primary">
            <AppleLogo className="inline -mt-[3px] mr-1.5" fill="white" />
            Download
          </Button>
          <Button className="cursor-pointer" variant="gray">
            <GithubLogo className="inline -mt-[3px] mr-1.5" fill="white" />
            Star on GitHub
          </Button>
        </div>
        <p className="mt-3 text-xs text-center text-gray-500">
          Available on macOS (Intel & Apple Silicon), Windows and Linux.
          <br />
          Coming soon to iOS & Android.
        </p>

        <iframe
          className="z-30 mt-20 border rounded-lg shadow-2xl bg-gray-850 border-gray-550"
          width={1200}
          height={600}
          src="http://localhost:8002?library_id=9068c6ec-cf90-451b-bb30-4174781e7bc6"
        />
        <Section
          orientation="right"
          heading="It's one big catalogue"
          description="Using content addressable storage in a virtual distributed filesystem, Spacedrive securely
    combines the storage capacity and processing power of your devices into one."
        />
        <Section
          orientation="left"
          heading="It's one big catalogue"
          description="Using content addressable storage in a virtual distributed filesystem, Spacedrive securely
    combines the storage capacity and processing power of your devices into one."
        />
        <Section
          orientation="right"
          heading="It's one big catalogue"
          description="Using content addressable storage in a virtual distributed filesystem, Spacedrive securely
    combines the storage capacity and processing power of your devices into one."
        />
        <div className="h-[500px]" />
      </div>
      <Bubbles />
    </div>
  );
}

export default App;
