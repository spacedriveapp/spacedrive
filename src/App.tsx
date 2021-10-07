import React, { useRef } from 'react';
import { Route, BrowserRouter as Router, Switch } from 'react-router-dom';
import { Sidebar } from './components/file/Sidebar';
import { TopBar } from './components/layout/TopBar';
import { useInputState } from './hooks/useInputState';
import { SettingsScreen } from './screens/Settings';
import { ExplorerScreen } from './screens/Explorer';

export default function App() {
  return (
    <Router>
      <div className="flex flex-col h-screen rounded-xl border border-gray-200 dark:border-gray-600 bg-white text-gray-900 dark:text-white dark:bg-gray-800 overflow-hidden ">
        <TopBar />
        <div className="flex flex-row min-h-full">
          <Sidebar />
          <div className="w-full flex">
            <Switch>
              <Route path="/settings">
                <SettingsScreen />
              </Route>
              <Route path="/explorer">
                <ExplorerScreen />
              </Route>
            </Switch>
          </div>
        </div>
      </div>
    </Router>
  );
}
