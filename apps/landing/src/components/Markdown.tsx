import clsx from 'clsx';
import Prism from 'prismjs';
import 'prismjs/components/prism-rust';
import 'prismjs/components/prism-typescript';
import { PropsWithChildren, useEffect } from 'react';
import '../atom-one.css';

interface MarkdownPageProps {
	classNames?: string;
	articleClassNames?: string;
}

function MarkdownPage(props: PropsWithChildren<MarkdownPageProps>) {
	useEffect(() => {
		Prism.highlightAll();
	}, []);

	return (
		<div className={clsx('mb-10 p-4', props.classNames)}>
			<article
				id="content"
				className={clsx(
					'lg:prose-xs prose prose-h1:text-[3.25em] prose-a:text-primary prose-a:no-underline prose-blockquote:rounded prose-blockquote:bg-gray-600 prose-code:rounded-md prose-code:bg-gray-650 prose-code:p-1 prose-code:font-normal prose-code:text-gray-400 prose-code:before:hidden prose-code:after:hidden prose-table:border-b prose-table:border-gray-500 prose-tr:even:bg-gray-700 prose-th:p-2 prose-td:border-l prose-td:border-gray-500 prose-td:p-2 prose-td:last:border-r prose-img:rounded dark:prose-invert text-[15px] sm:text-[16px]',
					props.articleClassNames
				)}
			>
				{props.children}
			</article>
		</div>
	);
}

export default MarkdownPage;
