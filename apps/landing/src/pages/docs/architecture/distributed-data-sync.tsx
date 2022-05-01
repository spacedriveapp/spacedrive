import Markdown from '../../../components/Markdown';
import React from 'react';
import { ReactComponent as Content } from '~/docs/architecture/distributed-data-sync.md';
import { Helmet } from 'react-helmet';

function Page() {
  return (
    <Markdown>
      <Helmet>
        <title>Distributed Data Sync - Spacedrive Documentation</title>
        <meta
          name="description"
          content="How we handle data sync with SQLite in a distributed network."
        />
      </Helmet>
      <Content />
    </Markdown>
  );
}

export default Page;
