import ReactMarkdown from 'react-markdown';
import rehypeRaw from 'rehype-raw';
import remarkGfm from 'remark-gfm';

export function Markdown({children, className}: {children: string; className?: string}) {
	return (
		<div className={className}>
			<ReactMarkdown
				remarkPlugins={[remarkGfm]}
				rehypePlugins={[rehypeRaw]}
				components={{
					p: ({children}) => <p className="my-1 first:mt-0 last:mb-0">{children}</p>,
					strong: ({children}) => <strong className="font-semibold text-ink">{children}</strong>,
					em: ({children}) => <em className="italic">{children}</em>,
					a: ({children, href, ...props}) => (
						<a
							href={href}
							target="_blank"
							rel="noopener noreferrer"
							className="text-accent underline decoration-accent/30 underline-offset-2 hover:decoration-accent/60"
							{...props}
						>
							{children}
						</a>
					),
					code: ({children, className}) => {
						const isBlock = Boolean(className);
						if (isBlock) {
							return <code className={className}>{children}</code>;
						}

						return (
							<code className="rounded bg-app-box px-1.5 py-0.5 font-mono text-[0.9em] text-ink">
								{children}
							</code>
						);
					},
					pre: ({children}) => (
						<pre className="border-app-line my-2 overflow-x-auto rounded-xl border bg-app-box px-3 py-2.5 text-[13px] leading-6">
							{children}
						</pre>
					),
					ul: ({children}) => <ul className="my-1.5 list-disc pl-5">{children}</ul>,
					ol: ({children}) => <ol className="my-1.5 list-decimal pl-5">{children}</ol>,
					li: ({children}) => <li className="my-0.5">{children}</li>,
					blockquote: ({children}) => (
						<blockquote className="border-app-line text-ink-dull my-2 border-l-2 pl-3 italic">
							{children}
						</blockquote>
					),
					h1: ({children}) => <h1 className="mt-3 mb-1 text-lg font-semibold text-ink">{children}</h1>,
					h2: ({children}) => <h2 className="mt-3 mb-1 text-base font-semibold text-ink">{children}</h2>,
					h3: ({children}) => <h3 className="mt-3 mb-1 text-sm font-semibold text-ink">{children}</h3>,
					h4: ({children}) => <h4 className="mt-3 mb-1 text-sm font-semibold text-ink">{children}</h4>,
					hr: () => <hr className="border-app-line my-3" />,
					table: ({children}) => <table className="my-2 w-full text-sm">{children}</table>,
					th: ({children}) => (
						<th className="border-app-line bg-app-box border px-2 py-1 text-left font-medium text-ink">
							{children}
						</th>
					),
					td: ({children}) => <td className="border-app-line border px-2 py-1">{children}</td>
				}}
			>
				{children}
			</ReactMarkdown>
		</div>
	);
}
