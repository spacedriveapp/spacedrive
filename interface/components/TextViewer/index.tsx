import clsx from 'clsx';
import Prism from 'prismjs';
import { memo, useEffect, useRef, useState } from 'react';

import * as prism from './prism';

export interface TextViewerProps {
	src: string;
	onLoad?: (event: HTMLElementEventMap['load']) => void;
	onError?: (event: HTMLElementEventMap['error']) => void;
	codeExtension?: string;
}

/// Large DOM nodes slow down rendering so we break up the text file into a `span` for every line
/// Doing too many dom manipulations at once will lag out the UI and specically the open modal animation
/// So we append a chunk of lines to the DOM and then timeout before appending the next chunk
const noLinesInRenderChunk = 1000;

const awaitSleep = (ms: number) => new Promise((resolve) => setTimeout(resolve, ms));

// TODO: ANSI support

export const TextViewer = memo(({ src, onLoad, onError, codeExtension }: TextViewerProps) => {
	const codeRef = useRef<HTMLPreElement>(null);
	const linesRef = useRef<HTMLPreElement>(null);

	useEffect(() => {
		// Ignore empty urls
		if (!src || src === '#') return;

		if (codeRef.current) codeRef.current.innerHTML = '';
		if (linesRef.current) linesRef.current.innerHTML = '';

		const controller = new AbortController();
		fetch(src, { mode: 'cors', signal: controller.signal })
			.then(async (response) => {
				if (!response.ok) throw new Error(`Invalid response: ${response.statusText}`);
				if (!response.body) return;
				onLoad?.(new UIEvent('load', {}));

				let firstChunk = true;

				// This code is not pretty but be careful when changing it
				// Download a GH Actions log from our Mobile CI workflow (around 12MB) and test on it!
				const reader = response.body.pipeThrough(new TextDecoderStream()).getReader();
				const renderLines = async () => {
					const { done, value } = await reader.read();
					if (done) return;

					const chunks = value.split('\n');
					let lineNo = 0;
					for (
						let chunkI = 0;
						chunkI < chunks.length;
						chunkI += firstChunk ? 200 : noLinesInRenderChunk
					) {
						const noInChunk = Math.min(
							chunks.length - chunkI,
							firstChunk ? 200 : noLinesInRenderChunk
						);
						console.log(noInChunk, chunks.length - chunkI, chunkI, chunks.length);

						const group = document.createElement('span');
						group.setAttribute('style', 'white-space: pre;');
						group.textContent =
							chunks.slice(chunkI, chunkI + noInChunk).join('\r\n') + '\r\n';

						let cb: IntersectionObserverCallback = (events) => {
							for (const event of events) {
								if (
									!event.isIntersecting ||
									group.getAttribute('data-highlighted') === 'true'
								)
									continue;
								group.setAttribute('data-highlighted', 'true');
								Prism.highlightElement(event.target, false); // Prism's async seems to be broken
							}
						};

						// We delay the syntax highlighter of the first chunk so that the modal animation doesn't lag.
						if (firstChunk) {
							firstChunk = false;
							const oldCb = cb;
							cb = (events, observer) => {
								setTimeout(() => oldCb(events, observer), 150);
							};
						}

						// We use an `IntersectionObserver` to only syntax highlight the visible portions of the document
						new IntersectionObserver(cb).observe(group);
						codeRef.current?.append(group);

						linesRef.current?.append(
							...[...Array(noInChunk)].map((_, i) => {
								const line = document.createElement('span');
								line.textContent = `${i + lineNo + 1}`;
								line.className = clsx(
									'token block text-end',
									i % 2 && 'bg-black/40'
								);
								return line;
							})
						);
						lineNo += noInChunk;

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
	}, [src, onError, onLoad, codeExtension, codeRef]);

	return (
		<pre
			tabIndex={0}
			className={clsx(
				'flex h-full w-full overflow-y-scroll',
				codeExtension && 'relative !pl-[3.8em]'
			)}
		>
			{codeExtension && (
				<span
					ref={linesRef}
					className="pointer-events-none absolute left-0 w-[3em] select-none text-[100%] tracking-[-1px] text-ink-dull"
				/>
			)}
			<code
				ref={codeRef}
				tabIndex={0}
				className={clsx(
					'relative whitespace-pre text-ink',
					codeExtension &&
						`language-${prism.languageMapping.get(codeExtension) ?? codeExtension}`
				)}
			/>
		</pre>
	);
});
