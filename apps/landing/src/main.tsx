import React, { Suspense } from 'react';
import ReactDOM from 'react-dom';
import { BrowserRouter as Router, useRoutes } from 'react-router-dom';

import routes from '~react-pages';
import NavBar from './components/NavBar';
import { Footer } from './components/Footer';

import '@sd/ui/style';
import './style.scss';

function App() {
  return (
    <Suspense fallback={<p>Loading...</p>}>
      <div className="dark:bg-black dark:text-white ">
        <NavBar />
        <div className="container z-10 flex flex-col items-center px-4 mx-auto overflow-x-hidden sm:overflow-x-visible ">
          {useRoutes(routes)}
          <Footer />
        </div>
      </div>
    </Suspense>
  );
}

ReactDOM.render(
  <React.StrictMode>
    <Router>
      <App />
    </Router>
  </React.StrictMode>,
  document.getElementById('root')
);
