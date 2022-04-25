import Markdown from '../components/Markdown';
import React from 'react';
import { ReactComponent as Content } from '../../../../docs/architecture/distributed-data-sync.md';
import { Helmet } from 'react-helmet';

function Page() {
  return (
    <Markdown>
      <Helmet>
        <title>Changelog - Spacedrive</title>
        <meta name="description" content="Updates and release builds of the Spacedrive app." />
      </Helmet>
      <Content />
    </Markdown>
  );
}

export default Page;
