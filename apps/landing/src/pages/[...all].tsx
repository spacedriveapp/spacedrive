import Markdown from '../components/Markdown';
import React from 'react';
import { Helmet } from 'react-helmet';
import { Button } from '@sd/ui';
import { SmileyXEyes } from 'phosphor-react';

function Page() {
  return (
    <Markdown>
      <Helmet>
        <title>Not Found - Spacedrive</title>
      </Helmet>
      <div className="flex flex-col items-center">
        <SmileyXEyes className="mb-3 w-44 h-44" />
        <h1 className="mb-2 text-center">In the quantum realm this page potentially exists.</h1>
        <p>In other words, thats a 404.</p>
        <div className="flex flex-wrap justify-center">
          <Button
            href={document.referrer || 'javascript:history.back()'}
            className="mt-2 mr-3 cursor-pointer "
            variant="gray"
          >
            ← Back
          </Button>
          <Button href="/" className="mt-2 cursor-pointer" variant="primary">
            Discover Spacedrive →
          </Button>
        </div>
      </div>
      <div className="h-96" />
    </Markdown>
  );
}

export default Page;
