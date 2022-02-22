import React, {useContext, useEffect, useState} from 'react';
import { Route, BrowserRouter as Router, Switch, Redirect } from 'react-router-dom';
import { Sidebar } from './components/file/Sidebar';
import { TopBar } from './components/layout/TopBar';
import { SettingsScreen } from './screens/Settings';
import { ExplorerScreen } from './screens/Explorer';
import { invoke } from '@tauri-apps/api';
import { DebugGlobalStore } from './Debug';
import { useCoreEvents } from './hooks/useCoreEvents';
import { AppState, useAppState } from './store/global';
import { Button } from 'ui';
import { ErrorBoundary, FallbackProps } from 'react-error-boundary';
import { useLocationStore, Location } from './store/locations';
import { OverviewScreen } from './screens/Overview';
import { SpacesScreen } from './screens/Spaces';
import {createModal, Modal} from "./components/layout/Modal";

export const SettingsModal = createModal('settings');

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

export default function App() {
  useCoreEvents();
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
        <div className="flex flex-row h-screen overflow-hidden text-gray-900 bg-white border border-gray-200 select-none rounded-xl dark:border-gray-500 dark:text-white dark:bg-gray-650">
          <Modal {...SettingsModal}>
            <SettingsScreen />
          </Modal>
          <DebugGlobalStore />
          <Sidebar />
          <div className="flex flex-col w-full min-h-full">
            <TopBar />
            <div className="relative flex w-full">
              <Switch>
                <Route path="/overview">
                  <OverviewScreen />
                </Route>
                <Route path="/spaces">
                  <SpacesScreen />
                </Route>
                <Route path="/explorer">
                  <ExplorerScreen />
                </Route>

              </Switch>
            </div>
          </div>

        </div>
      </Router>
    </ErrorBoundary>
  );
}
