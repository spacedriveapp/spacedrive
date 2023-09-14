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

/// Large DOM nodes slow down rendering so we break up the text file into a `span` for every line
/// Doing too many dom manipulations at once will lag out the UI and specically the open modal animation
/// So we append a chunk of lines to the DOM and then timeout before appending the next chunk
const noLinesInRenderChunk = 1000;

const awaitSleep = (ms: number) => new Promise((resolve) => setTimeout(resolve, ms));

// TODO: ANSI support

export const TextViewer = memo(
	({ src, onLoad, onError, className, codeExtension }: TextViewerProps) => {
		const ref = useRef<HTMLPreElement>(null);
		const [highlight, setHighlight] = useState<{
			code: string;
			length: number;
			language: string;
		}>();

		useEffect(() => {
			// Ignore empty urls
			if (!src || src === '#') return;

			const controller = new AbortController();
			fetch(src, { mode: 'cors', signal: controller.signal })
				.then(async (response) => {
					if (!response.ok) throw new Error(`Invalid response: ${response.statusText}`);
					if (!response.body) return;
					onLoad?.(new UIEvent('load', {}));

					// This code is not pretty but be careful when changing it
					// Download a GH Actions log from our Mobile CI workflow (around 12MB) and test on it!
					const reader = response.body.pipeThrough(new TextDecoderStream()).getReader();
					const renderLines = async () => {
						const { done, value } = await reader.read();
						if (done) return;

						const chunks = value.split('\n');
						for (let i = 0; i < chunks.length; i += noLinesInRenderChunk) {
							ref.current?.append(
								...chunks.slice(i, i + noLinesInRenderChunk).map((line) => {
									const child = document.createElement('span');
									child.innerText = line;
									return child;
								})
							);

							await awaitSleep(500);
						}

						await renderLines();
					};
					renderLines();
				})
				.catch((error) => {
					if (!controller.signal.aborted)
						onError?.(new ErrorEvent('error', { message: `${error}` }));
				});

			return () => controller.abort();
		}, [src, onError, onLoad, codeExtension, ref]);

		return (
			<pre
				ref={ref}
				tabIndex={0}
				className={clsx(
					'flex flex-col overflow-y-scroll text-ink',
					className,
					highlight && ['relative !pl-[3.8em]', `language-${highlight.language}`]
				)}
			>
				{/* {highlight ? (
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
				)} */}
			</pre>
		);
	}
);
