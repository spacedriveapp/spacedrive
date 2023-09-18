import clsx from 'clsx';
import Prism from 'prismjs';
import { memo, useEffect, useRef, useState } from 'react';

import * as prism from './prism';

export interface TextViewerProps {
	src: string;
	onLoad?: (event: HTMLElementEventMap['load']) => void;
	onError?: (event: HTMLElementEventMap['error']) => void;
	className?: string;
	codeExtension?: string;
}

/// Large DOM nodes slow down rendering so we break up the text file into a `span` for every line
/// Doing too many dom manipulations at once will lag out the UI and specically the open modal animation
/// So we append a chunk of lines to the DOM and then timeout before appending the next chunk
const noLinesInRenderChunk = 1000;

const awaitSleep = (ms: number) => new Promise((resolve) => setTimeout(resolve, ms));

// TODO: ANSI support

export const TextViewer = memo(
	({ src, onLoad, onError, className, codeExtension }: TextViewerProps) => {
		const ref = useRef<HTMLPreElement>(null);

		useEffect(() => {
			// Ignore empty urls
			if (!src || src === '#') return;

			if (ref.current && codeExtension)
				ref.current.className += ` language-${
					prism.languageMapping.get(codeExtension) ?? codeExtension
				}`;

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
						for (
							let i = 0;
							i < chunks.length;
							i += firstChunk ? 200 : noLinesInRenderChunk
						) {
							const group = document.createElement('span');
							group.setAttribute('style', 'white-space: pre;');
							group.textContent =
								chunks
									.slice(i, i + (firstChunk ? 200 : noLinesInRenderChunk))
									.join('\r\n') + '\r\n';

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

							if (firstChunk) {
								firstChunk = false;
								const oldCb = cb;
								cb = (events, observer) => {
									setTimeout(() => oldCb(events, observer), 150);
								};
							}

							new IntersectionObserver(cb).observe(group);
							ref.current?.append(group);

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
			<pre className="h-full w-full">
				<code
					ref={ref}
					tabIndex={0}
					className={clsx(
						'flex h-full w-full flex-col overflow-y-scroll whitespace-pre text-ink',
						className
					)}
				/>
			</pre>
		);
	}
);
