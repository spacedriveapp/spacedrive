import React, { useEffect } from 'react';

import Prism from 'prismjs';
import 'prismjs/components/prism-rust';
import '../atom-one.css';

interface MarkdownPageProps {
  children: React.ReactNode;
}

function MarkdownPage(props: MarkdownPageProps) {
  useEffect(() => {
    Prism.highlightAll();
  }, []);
  return (
    <div className="container max-w-4xl p-4 mt-32 mb-20">
      <article id="content" className="m-auto prose lg:prose-xs dark:prose-invert">
        {props.children}
      </article>
    </div>
  );
}

export default MarkdownPage;
