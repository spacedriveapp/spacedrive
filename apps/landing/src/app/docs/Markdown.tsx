import clsx from 'clsx';
import { PropsWithChildren } from 'react';

interface MarkdownPageProps {
	classNames?: string;
	articleClassNames?: string;
}

export function Markdown(props: PropsWithChildren<MarkdownPageProps>) {
	return (
		<div className={clsx('mb-10 px-8 py-4', props.classNames)}>
			<article
				id="content"
				className={clsx(
					'lg:prose-xs prose prose-invert text-[15px] prose-h1:text-[3.25em] prose-a:text-primary prose-a:no-underline prose-blockquote:rounded prose-blockquote:bg-gray-600 prose-code:rounded-md prose-code:bg-gray-650 prose-code:p-1 prose-code:font-normal prose-code:text-gray-400 prose-code:before:hidden prose-code:after:hidden prose-table:border-b prose-table:border-gray-500 prose-tr:even:bg-gray-700 prose-th:p-2 prose-td:border-l prose-td:border-gray-500 prose-td:p-2 prose-td:last:border-r prose-img:rounded sm:text-[16px]',
					props.articleClassNames
				)}
			>
				{props.children}
			</article>
		</div>
	);
}
