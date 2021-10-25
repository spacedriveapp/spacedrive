import React, { useEffect, useRef } from 'react';
import { Route, BrowserRouter as Router, Switch, Redirect } from 'react-router-dom';
import { Sidebar } from './components/file/Sidebar';
import { TopBar } from './components/layout/TopBar';
import { useInputState } from './hooks/useInputState';
import { SettingsScreen } from './screens/Settings';
import { ExplorerScreen } from './screens/Explorer';
import { invoke } from '@tauri-apps/api';
import { DebugGlobalStore } from './store/Debug';
import { useGlobalEvents } from './hooks/useGlobalEvents';
import { AppState, useAppState } from './store/app';
import { Modal } from './components/layout/Modal';
import { useKey, useKeyBindings } from 'rooks';
// import { useHotkeys } from 'react-hotkeys-hook';

export default function App() {
  useGlobalEvents();
  useEffect(() => {
    invoke<AppState>('get_config').then((state) => useAppState.getState().update(state));
  }, []);

  // useHotkeys('command+q', () => {
  //   process.exit();
  // });

  return (
    <Router>
      <div className="flex flex-col select-none h-screen rounded-xl border border-gray-200 dark:border-gray-450 bg-white text-gray-900 dark:text-white dark:bg-gray-800 overflow-hidden ">
        <DebugGlobalStore />
        <TopBar />
        <div className="flex flex-row min-h-full">
          <Sidebar />
          <div className="relative w-full flex bg-gray-50 dark:bg-gray-800">
            <Switch>
              <Route exact path="/">
                <Redirect to="/explorer" />
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
        {/* <Modal /> */}
      </div>
    </Router>
  );
}
