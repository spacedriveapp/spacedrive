import clsx from 'clsx';
import { memo, useEffect, useRef, useState } from 'react';

import { highlight } from './worker';

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
		const lineNoRef = useRef<HTMLSpanElement>(null);

		useEffect(() => {
			// Ignore empty urls
			if (!src || src === '#') return;

			const controller = new AbortController();
			fetch(src, { mode: 'cors', signal: controller.signal })
				.then(async (response) => {
					if (!response.ok) throw new Error(`Invalid response: ${response.statusText}`);
					if (!response.body) return;
					onLoad?.(new UIEvent('load', {}));

					// We wanna do reactive updates to avoid a complete rerender which will cause the `fetch` to be restarted
					let updatedClasses = false;

					// This code is not pretty but be careful when changing it
					// Download a GH Actions log from our Mobile CI workflow (around 12MB) and test on it!
					const reader = response.body.pipeThrough(new TextDecoderStream()).getReader();
					const renderLines = async () => {
						const { done, value } = await reader.read();
						if (done) return;

						const chunks = value.split('\n');
						for (let i = 0; i < chunks.length; i += noLinesInRenderChunk) {
							let totalRenderedLines = 0;
							ref.current?.append(
								...(await Promise.all(
									chunks.slice(i, i + noLinesInRenderChunk).map(async (line) => {
										totalRenderedLines++;

										const child = document.createElement('span');

										if (line === '') {
											child.append(document.createElement('br'));
										} else if (codeExtension) {
											const x = performance.now();

											// TODO: Worker or directly?
											const result = await worker.highlight(
												line,
												codeExtension
											);

											console.log(performance.now() - x);

											if (result) {
												child.innerHTML = result.code;

												if (!updatedClasses && ref.current) {
													updatedClasses = true;
													ref.current.className += `relative !pl-[3.8em] language-${result.language}`;
												}
											}
										} else {
											child.innerText = line;
										}
										return child;
									})
								))
							);

							// TODO: This breaks the explorer sidebar preview -> Can it not use this and only load the first chunk of the file
							// for (let i = 0; i < totalRenderedLines; i += 1) {
							// 	const lineNo = document.createElement('span');
							// 	lineNo.className =
							// 		'token block text-end' + (i % 2 ? ' bg-black/40' : '');
							// 	lineNo.textContent = i.toString();
							// 	lineNoRef.current?.append(lineNo);
							// }

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

		const inner = <code ref={ref} />;

		return (
			<pre
				ref={ref}
				tabIndex={0}
				className={clsx('flex flex-col overflow-y-scroll text-ink', className)}
			>
				{codeExtension ? (
					<>
						<span
							ref={lineNoRef}
							className="pointer-events-none absolute left-0 top-[1em] w-[3em] select-none text-[100%] tracking-[-1px] text-ink-dull"
						/>

						{inner}
					</>
				) : (
					inner
				)}
			</pre>
		);
	}
);
