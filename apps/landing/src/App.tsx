import { Button } from '@sd/ui';
import { Bubbles } from './Bubbles';
import './index.css';

import NavBar from './components/NavBar';
import { Footer } from './components/Footer';
import { Apple, Github, Linux, Windows } from '@icons-pack/react-simple-icons';
import { useState } from 'react';

interface SectionProps {
  orientation: 'left' | 'right';
  heading?: string;
  description?: string | React.ReactNode;
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
    <div className="grid grid-cols-1 lg:grid-cols-2 my-44">
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
  const [showApp, setShowApp] = useState(true);
  return (
    <div>
      <NavBar />
      <div className="container z-10 flex flex-col items-center px-4 mx-auto text-white bg-black">
        {/* <AppLogo className="z-30 w-40 h-40 mt-32" /> */}
        <h1 className="text-4xl font-black leading-snug text-center md:text-6xl mt-36">
          {/* One space, all your drives. */}A file explorer from the future.
        </h1>
        <p className="max-w-4xl mt-1 mb-8 text-lg text-center text-gray-450">
          Combine your drives and clouds into one database that you can organize and explore from
          any device.
          <br />
          Designed for creators, hoarders and the painfully disorganized.
        </p>
        <div className="flex flex-row space-x-4">
          {/* <Button className="px-2">
          <WindowsLogo className="" fill="white" />
        </Button> */}

          <Button
            onClick={() =>
              alert(
                "You're here early! This is the only button on this page that does not work, I promise. Release build coming very soon—follow @spacedriveapp for updates."
              )
            }
            className="cursor-pointer"
            variant="primary"
          >
            Download
          </Button>

          <a href="https://github.com/spacedriveapp/spacedrive" target="_blank">
            <Button className="cursor-pointer" variant="gray">
              <Github className="inline w-5 h-5 -mt-[4px] -ml-1 mr-2" fill="white" />
              Star on GitHub
            </Button>
          </a>
        </div>
        <p className="mt-3 text-xs text-center text-gray-500">
          Available on macOS (Intel & Apple Silicon), Windows and Linux.
          <br />
          Coming soon to iOS & Android.
        </p>

        {showApp && (
          <iframe
            className="z-30 mt-20 border rounded-lg shadow-2xl bg-gray-850 border-gray-550"
            width={1200}
            height={600}
            onError={(e) => {
              setShowApp(false);
            }}
            src="http://localhost:8002?library_id=9068c6ec-cf90-451b-bb30-4174781e7bc6"
          />
        )}
        <Section
          orientation="right"
          heading="Never leave a file behind."
          description={
            <>
              Spacedrive accounts for every file you own, uniquely fingerprinting and extracting
              metadata so you can sort, tag, backup and share files without limitations of any one
              cloud provider.
              <br />
              <br />
              <a
                className="transition text-primary-600 hover:text-primary-500"
                href="https://github.com/spacedriveapp"
                target="_blank"
              >
                Find out more →
              </a>
            </>
          }
        />
        <Footer />
      </div>
      <Bubbles />
    </div>
  );
}

export default App;
