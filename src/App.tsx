import React, { useEffect, useRef } from 'react';
import { Route, BrowserRouter as Router, Switch, Redirect } from 'react-router-dom';
import { Sidebar } from './components/file/Sidebar';
import { TopBar } from './components/layout/TopBar';
import { useInputState } from './hooks/useInputState';
import { SettingsScreen } from './screens/Settings';
import { ExplorerScreen } from './screens/Explorer';
import { invoke } from '@tauri-apps/api';
import { DebugGlobalStore } from './Debug';
import { useGlobalEvents } from './hooks/useGlobalEvents';
import { AppState, useAppState } from './store/global';
import { Modal } from './components/layout/Modal';
import { useKey, useKeyBindings } from 'rooks';
// import { useHotkeys } from 'react-hotkeys-hook';
import { ErrorBoundary, FallbackProps } from 'react-error-boundary';
import { Button } from './components/primative';
import { useLocationStore, Location } from './store/locations';
import { OverviewScreen } from './screens/Overview';
import { SpacesScreen } from './screens/Spaces';

function ErrorFallback({ error, resetErrorBoundary }: FallbackProps) {
  return (
    <div
      data-tauri-drag-region
      role="alert"
      className="flex border border-gray-200 dark:border-gray-650 h-screen justify-center items-center flex-col rounded-lg w-screen bg-gray-50 dark:bg-gray-950 dark:text-white p-4"
    >
      <p className="text-sm m-3 text-gray-400 font-bold">APP CRASHED</p>
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

export default function App() {
  useGlobalEvents();
  useEffect(() => {
    invoke<AppState>('get_config').then((state) => useAppState.getState().update(state));
    invoke<Location[]>('get_mounts').then((locations) =>
      useLocationStore.getState().setLocations(locations)
    );
  }, []);

  // useHotkeys('command+q', () => {
  //   process.exit();
  // });

  return (
    <ErrorBoundary
      FallbackComponent={ErrorFallback}
      onReset={() => {
        // reset the state of your app so the error doesn't happen again
      }}
    >
      <Router>
        <div className="flex flex-col select-none h-screen rounded-xl border border-gray-200 dark:border-gray-550 bg-white text-gray-900 dark:text-white dark:bg-gray-800 overflow-hidden">
          <DebugGlobalStore />
          <TopBar />
          <div className="flex flex-row min-h-full">
            <Sidebar />
            <div className="relative w-full flex bg-gray-50 dark:bg-gray-800">
              <Switch>
                <Route exact path="/">
                  <Redirect to="/explorer" />
                </Route>
                <Route path="/overview">
                  <OverviewScreen />
                </Route>
                <Route path="/spaces">
                  <SpacesScreen />
                </Route>
                <Route path="/explorer">
                  <ExplorerScreen />
                </Route>
                <Route path="/settings">
                  <SettingsScreen />
                </Route>
              </Switch>
            </div>
          </div>
          <Modal />
        </div>
      </Router>
    </ErrorBoundary>
  );
}
