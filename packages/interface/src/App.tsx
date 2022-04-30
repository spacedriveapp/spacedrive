import React, { useContext, useEffect, useState } from 'react';
import {
  MemoryRouter,
  Location,
  Outlet,
  Route,
  Routes,
  useLocation,
  useNavigate
} from 'react-router-dom';
import { Sidebar } from './components/file/Sidebar';
import { SettingsScreen } from './screens/Settings';
import { ExplorerScreen } from './screens/Explorer';
import { useCoreEvents } from './hooks/useCoreEvents';
import { ErrorBoundary, FallbackProps } from 'react-error-boundary';
import { OverviewScreen } from './screens/Overview';
import { DebugScreen } from './screens/Debug';
import { Modal } from './components/layout/Modal';
import GeneralSettings from './screens/settings/GeneralSettings';
import SlideUp from './components/transitions/SlideUp';
import SecuritySettings from './screens/settings/SecuritySettings';
import LocationSettings from './screens/settings/LocationSettings';
import { RedirectPage } from './screens/Redirect';
import { QueryClient, QueryClientProvider } from 'react-query';
import { BaseTransport, ClientProvider, setTransport } from '@sd/client';
import { Button } from '@sd/ui';
import { CoreEvent } from '@sd/core';
import clsx from 'clsx';
import './style.scss';
import { ContentScreen } from './screens/Content';
import LibrarySettings from './screens/settings/LibrarySettings';

import '@fontsource/inter/variable.css';
import { TagScreen } from './screens/Tag';

const queryClient = new QueryClient();

export const AppPropsContext = React.createContext<AppProps | null>(null);

export type Platform = 'browser' | 'macOS' | 'windows' | 'linux';

export interface AppProps {
  transport: BaseTransport;
  platform: Platform;
  convertFileSrc: (url: string) => string;
  openDialog: (options: { directory?: boolean }) => Promise<string | string[]>;
  onClose?: () => void;
  onMinimize?: () => void;
  onFullscreen?: () => void;
  useMemoryRouter: boolean;
}

function AppLayout() {
  const appPropsContext = useContext(AppPropsContext);
  const [isWindowRounded, setIsWindowRounded] = useState(false);
  const [hasWindowBorder, setHasWindowBorder] = useState(true);

  useEffect(() => {
    if (appPropsContext?.platform === 'macOS') {
      setIsWindowRounded(true);
    }
    if (appPropsContext?.platform === 'browser') {
      setHasWindowBorder(false);
    }
    else {
      setIsWindowRounded(false);
    }
  }, []);

  return (
    <div
      className={clsx(
        'flex flex-row h-screen overflow-hidden text-gray-900 bg-white select-none dark:text-white dark:bg-gray-650',
        isWindowRounded && 'rounded-xl',
        hasWindowBorder && 'border border-gray-200 dark:border-gray-500'
      )}
    >
      <Sidebar />
      <div className="flex flex-col w-full min-h-full">
        {/* <TopBar /> */}

        <div className="relative flex w-full">
          <Outlet />
        </div>
      </div>
    </div>
  );
}

function SettingsRoutes({ modal = false }) {
  return (
    <SlideUp>
      <Routes>
        <Route
          path={modal ? '/settings' : '/'}
          element={modal ? <Modal children={<SettingsScreen />} /> : <SettingsScreen />}
        >
          <Route index element={<GeneralSettings />} />
          <Route path="general" element={<GeneralSettings />} />
          <Route path="security" element={<SecuritySettings />} />
          <Route path="appearance" element={<></>} />
          <Route path="locations" element={<LocationSettings />} />
          <Route path="library" element={<LibrarySettings />} />
          <Route path="media" element={<></>} />
          <Route path="keys" element={<></>} />
          <Route path="tags" element={<></>} />
          <Route path="sync" element={<></>} />
          <Route path="contacts" element={<></>} />
        </Route>
      </Routes>
    </SlideUp>
  );
}

function Router() {
  let location = useLocation();
  let state = location.state as { backgroundLocation?: Location };

  useEffect(() => {
    console.log({ url: location.pathname });
  }, [state]);

  return (
    <>
      <Routes location={state?.backgroundLocation || location}>
        <Route path="/" element={<AppLayout />}>
          <Route index element={<RedirectPage to="/overview" />} />
          <Route path="overview" element={<OverviewScreen />} />
          <Route path="content" element={<ContentScreen />} />
          <Route path="debug" element={<DebugScreen />} />
          <Route path="settings/*" element={<SettingsRoutes />} />
          <Route path="explorer/:id" element={<ExplorerScreen />} />
          <Route path="tag/:id" element={<TagScreen />} />
          <Route path="*" element={<NotFound />} />
        </Route>
      </Routes>
      {state?.backgroundLocation && <SettingsRoutes modal />}
    </>
  );
}

function ErrorFallback({ error, resetErrorBoundary }: FallbackProps) {
  return (
    <div
      data-tauri-drag-region
      role="alert"
      className="flex flex-col items-center justify-center w-screen h-screen p-4 border border-gray-200 rounded-lg dark:border-gray-650 bg-gray-50 dark:bg-gray-650 dark:text-white"
    >
      <p className="m-3 text-sm font-bold text-gray-400">APP CRASHED</p>
      <h1 className="text-2xl font-bold">We're past the event horizon...</h1>
      <pre className="m-2">Error: {error.message}</pre>
      <div className="flex flex-row space-x-2">
        <Button variant="primary" className="mt-2" onClick={resetErrorBoundary}>
          Reload
        </Button>
        <Button className="mt-2" onClick={resetErrorBoundary}>
          Send report
        </Button>
      </div>
    </div>
  );
}

function NotFound() {
  const navigate = useNavigate();
  return (
    <div
      data-tauri-drag-region
      role="alert"
      className="flex flex-col items-center justify-center w-full h-full p-4 rounded-lg dark:text-white"
    >
      <p className="m-3 mt-20 text-sm font-semibold text-gray-500 uppercase">Error: 404</p>
      <h1 className="text-4xl font-bold">You chose nothingness.</h1>
      <div className="flex flex-row space-x-2">
        <Button variant="primary" className="mt-4" onClick={() => navigate(-1)}>
          Go Back
        </Button>
      </div>
    </div>
  );
}

function MemoryRouterContainer() {
  useCoreEvents();
  return (
    <MemoryRouter>
      <Router />
    </MemoryRouter>
  );
}

function BrowserRouterContainer() {
  useCoreEvents();
  return (
    <MemoryRouter>
      <Router />
    </MemoryRouter>
  );
}

export function bindCoreEvent() {}

export default function App(props: AppProps) {
  // @ts-ignore: TODO: This is a hack and a better solution should probably be found. This exists so that the queryClient can be accessed within the subpackage '@sd/client'. Refer to <ClientProvider /> for where this is used.
  if (window.ReactQueryClient === undefined) {
    // @ts-ignore
    window.ReactQueryClient = queryClient;
  }

  setTransport(props.transport);

  console.log('App props', props);

  return (
    <>
      {/* @ts-ignore */}
      <ErrorBoundary FallbackComponent={ErrorFallback} onReset={() => {}}>
        {/* @ts-ignore */}
        <QueryClientProvider client={queryClient} contextSharing={false}>
          <AppPropsContext.Provider value={props}>
            <ClientProvider>
              {props.useMemoryRouter ? <MemoryRouterContainer /> : <BrowserRouterContainer />}
            </ClientProvider>
          </AppPropsContext.Provider>
        </QueryClientProvider>
      </ErrorBoundary>
    </>
  );
}
