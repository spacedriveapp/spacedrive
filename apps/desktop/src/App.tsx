import React, { useEffect } from 'react';
import {
  BrowserRouter,
  Location,
  Outlet,
  Route,
  Routes,
  useLocation,
  useNavigate
} from 'react-router-dom';
import { Sidebar } from './components/file/Sidebar';
import { TopBar } from './components/layout/TopBar';
import { SettingsScreen } from './screens/Settings';
import { ExplorerScreen } from './screens/Explorer';
import { invoke } from '@tauri-apps/api';
import { DebugGlobalStore } from './Debug';
import { useCoreEvents } from './hooks/useCoreEvents';
import { AppState, useAppState } from './store/global';
import { Button } from './components/primitive';
import { ErrorBoundary, FallbackProps } from 'react-error-boundary';
import { useLocationStore } from './store/locations';
import { OverviewScreen } from './screens/Overview';
import { SpacesScreen } from './screens/Spaces';
import { Modal } from './components/layout/Modal';
import GeneralSettings from './screens/settings/GeneralSettings';
import SlideUp from './components/transitions/SlideUp';
import SecuritySettings from './screens/settings/SecuritySettings';
import LocationSettings from './screens/settings/LocationSettings';
import { RedirectPage } from './screens/Redirect';

function AppLayout() {
  return (
    <div className="flex flex-row h-screen overflow-hidden text-gray-900 bg-white border border-gray-200 select-none rounded-xl dark:border-gray-500 dark:text-white dark:bg-gray-650">
      <Sidebar />
      <div className="flex flex-col w-full min-h-full">
        <TopBar />
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
          <Route path="media" element={<></>} />
          <Route path="keys" element={<></>} />
          <Route path="tags" element={<></>} />
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
          <Route path="spaces" element={<SpacesScreen />} />
          <Route path="settings/*" element={<SettingsRoutes />} />
          <Route path="explorer" element={<ExplorerScreen />} />
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
      className="flex flex-col items-center justify-center w-full h-full p-4 border border-gray-200 rounded-lg dark:border-gray-650 bg-gray-50 dark:bg-gray-650 dark:text-white"
    >
      <p className="m-3 text-sm font-bold text-gray-400">Error: 404</p>
      <h1 className="text-2xl font-bold">Not found bozo.</h1>
      <div className="flex flex-row space-x-2">
        <Button variant="primary" className="mt-2" onClick={() => navigate(-1)}>
          Go Back
        </Button>
      </div>
    </div>
  );
}

// useHotkeys('command+q', () => {
//   process.exit();
// });

export default function App() {
  useCoreEvents();

  useEffect(() => {
    invoke<AppState>('get_config').then((state) => useAppState.getState().update(state));
    invoke<Location[]>('get_mounts').then((locations) =>
      //@ts-expect-error
      useLocationStore.getState().setLocations(locations)
    );
  }, []);

  return (
    <ErrorBoundary
      FallbackComponent={ErrorFallback}
      // reset the state of your app so the error doesn't happen again
      onReset={() => {}}
    >
      <DebugGlobalStore />
      <BrowserRouter>
        <Router />
      </BrowserRouter>
    </ErrorBoundary>
  );
}
