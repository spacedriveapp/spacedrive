import clsx from 'clsx';
import Prism from 'prismjs';
import 'prismjs/components/prism-rust';
import { useEffect } from 'react';

import '../atom-one.css';

interface MarkdownPageProps {
	children: React.ReactNode;
	classNames?: string;
}

function MarkdownPage(props: MarkdownPageProps) {
	useEffect(() => {
		Prism.highlightAll();
	}, []);

	return (
		<div className={clsx('max-w-4xl p-4 mt-[105px] mb-20 sm:container', props.classNames)}>
			<article
				id="content"
				className="prose prose-blockquote:bg-red-500 prose-a:text-primary prose-a:no-underline lg:prose-xs dark:prose-invert prose-td:p-2 prose-th:p-2 prose-td:border-l prose-td:border-gray-500 prose-td:last:border-r prose-table:border-b prose-table:border-gray-500 prose-tr:even:bg-gray-700"
			>
				{props.children}
			</article>
		</div>
	);
}

export default MarkdownPage;
