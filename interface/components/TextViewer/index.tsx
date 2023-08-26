import clsx from 'clsx';
import { memo, useEffect, useRef, useState } from 'react';
import './prism.css';

export interface TextViewerProps {
	src: string;
	onLoad?: (event: HTMLElementEventMap['load']) => void;
	onError?: (event: HTMLElementEventMap['error']) => void;
	className?: string;
	codeExtension?: string;
}

// prettier-ignore
type Worker = typeof import('./worker')
export const worker = new ComlinkWorker<Worker>(new URL('./worker', import.meta.url));

const NEW_LINE_EXP = /\n(?!$)/g;

export const TextViewer = memo(
	({ src, onLoad, onError, className, codeExtension }: TextViewerProps) => {
		const ref = useRef<HTMLPreElement>(null);
		const [highlight, setHighlight] = useState<{
			code: string;
			length: number;
			language: string;
		}>();
		const [textContent, setTextContent] = useState('');

		useEffect(() => {
			// Ignore empty urls
			if (!src || src === '#') return;

			const controller = new AbortController();

			fetch(src, { mode: 'cors', signal: controller.signal })
				.then(async (response) => {
					if (!response.ok) throw new Error(`Invalid response: ${response.statusText}`);
					const text = await response.text();

					if (controller.signal.aborted) return;

					onLoad?.(new UIEvent('load', {}));
					setTextContent(text);

					if (codeExtension) {
						try {
							const env = await worker.highlight(text, codeExtension);
							if (env && !controller.signal.aborted) {
								const match = text.match(NEW_LINE_EXP);
								setHighlight({
									...env,
									length: (match ? match.length + 1 : 1) + 1
								});
							}
						} catch (error) {
							console.error(error);
						}
					}
				})
				.catch((error) => {
					if (!controller.signal.aborted)
						onError?.(new ErrorEvent('error', { message: `${error}` }));
				});

			return () => controller.abort();
		}, [src, onError, onLoad, codeExtension]);

		return (
			<pre
				ref={ref}
				tabIndex={0}
				className={clsx(
					className,
					highlight && ['relative !pl-[3.8em]', `language-${highlight.language}`]
				)}
			>
				{highlight ? (
					<>
						<span className="pointer-events-none absolute left-0 top-[1em] w-[3em] select-none text-[100%] tracking-[-1px] text-ink-dull">
							{Array.from(highlight, (_, i) => (
								<span
									key={i}
									className={clsx('token block text-end', i % 2 && 'bg-black/40')}
								>
									{i + 1}
								</span>
							))}
						</span>
						<code
							style={{ whiteSpace: 'inherit' }}
							className={clsx('relative', `language-${highlight.language}`)}
							dangerouslySetInnerHTML={{ __html: highlight.code }}
						/>
					</>
				) : (
					textContent
				)}
			</pre>
		);
	}
);
