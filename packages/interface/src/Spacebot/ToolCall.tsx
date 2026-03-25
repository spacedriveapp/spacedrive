import {CaretDown, Check, Play, X} from '@phosphor-icons/react';
import clsx from 'clsx';
import {useMemo, useState} from 'react';

import type {TranscriptStep} from '@spacebot/api-client';

export type ToolCallStatus = 'running' | 'completed' | 'error';

export interface ToolCallPair {
	id: string;
	name: string;
	argsRaw: string;
	args: Record<string, unknown> | null;
	resultRaw: string | null;
	result: Record<string, unknown> | null;
	status: ToolCallStatus;
}

export function pairTranscriptSteps(steps: TranscriptStep[]): ToolCallPair[] {
	const resultsById = new Map<string, {name: string; text: string}>();

	for (const step of steps) {
		if (step.type === 'tool_result') {
			resultsById.set(step.call_id, {name: step.name, text: step.text});
		}
	}

	const pairs: ToolCallPair[] = [];

	for (const step of steps) {
		if (step.type !== 'action') continue;

		for (const content of step.content) {
			if (content.type !== 'tool_call') continue;

			const result = resultsById.get(content.id);
			const parsedArgs = tryParseJson(content.args);
			const parsedResult = result ? tryParseJson(result.text) : null;
			const isError = result ? isErrorResult(result.text, parsedResult) : false;

			pairs.push({
				id: content.id,
				name: content.name,
				argsRaw: content.args,
				args: parsedArgs,
				resultRaw: result?.text ?? null,
				result: parsedResult,
				status: result ? (isError ? 'error' : 'completed') : 'running'
			});
		}
	}

	return pairs;
}

function tryParseJson(text: string): Record<string, unknown> | null {
	if (!text.trim()) return null;

	try {
		const parsed = JSON.parse(text);
		if (typeof parsed === 'object' && parsed !== null && !Array.isArray(parsed)) {
			return parsed as Record<string, unknown>;
		}
	} catch {}

	return null;
}

function isErrorResult(text: string, parsed: Record<string, unknown> | null): boolean {
	if (parsed?.error) return true;
	if (parsed?.status === 'error') return true;
	if (parsed?.success === false) return true;
	if (typeof parsed?.exit_code === 'number' && parsed.exit_code !== 0) return true;

	const lower = text.toLowerCase();
	return (
		lower.startsWith('error:') ||
		lower.startsWith('error -') ||
		lower.startsWith('failed:') ||
		lower.startsWith('toolset error:')
	);
}

function truncate(text: string, maxLen: number): string {
	return text.length <= maxLen ? text : `${text.slice(0, maxLen)}...`;
}

function formatToolName(name: string): string {
	const overrides: Record<string, string> = {
		set_status: 'Status',
		file_read: 'Read',
		file_write: 'Write',
		file_edit: 'Edit',
		file_list: 'List',
		shell: 'Shell'
	};

	if (overrides[name]) return overrides[name];

	return name
		.replace(/^browser_/, '')
		.replace(/^file_/, '')
		.split('_')
		.map((word) => word.charAt(0).toUpperCase() + word.slice(1))
		.join(' ');
}

function summaryForPair(pair: ToolCallPair): string | null {
	if (pair.name === 'shell') {
		const command = pair.args?.command;
		return command ? truncate(String(command), 56) : null;
	}

	if (pair.name.startsWith('file_')) {
		const path = pair.args?.path;
		return path ? truncate(String(path), 56) : null;
	}

	if (pair.name === 'set_status') {
		const message = pair.args?.message;
		return message ? truncate(String(message), 56) : null;
	}

	if (pair.argsRaw && pair.argsRaw !== '{}') {
		return truncate(pair.argsRaw, 56);
	}

	return null;
}

function statusIcon(status: ToolCallStatus) {
	if (status === 'running') return <Play className="size-3" weight="fill" />;
	if (status === 'error') return <X className="size-3" weight="bold" />;
	return <Check className="size-3" weight="bold" />;
}

function statusColor(status: ToolCallStatus) {
	if (status === 'running') return 'text-accent';
	if (status === 'error') return 'text-red-400';
	return 'text-emerald-400';
}

function ArgsView({pair}: {pair: ToolCallPair}) {
	if (pair.name === 'shell' && pair.args?.command) {
		return (
			<pre className="max-h-40 overflow-auto whitespace-pre-wrap px-3 py-2 font-mono text-[11px] text-ink-dull">
				<span className="select-none text-ink-faint">$ </span>
				{String(pair.args.command)}
			</pre>
		);
	}

	if (pair.args && Object.keys(pair.args).length > 0) {
		return (
			<div className="flex flex-col gap-1 px-3 py-2">
				{Object.entries(pair.args).map(([key, value]) => (
					<p key={key} className="text-[11px]">
						<span className="text-ink-faint">{key}: </span>
						<span className="text-ink-dull">{typeof value === 'string' ? truncate(value, 180) : JSON.stringify(value)}</span>
					</p>
				))}
			</div>
		);
	}

	if (!pair.argsRaw || pair.argsRaw === '{}') return null;

	return (
		<pre className="max-h-40 overflow-auto whitespace-pre-wrap px-3 py-2 font-mono text-[11px] text-ink-dull">
			{pair.argsRaw}
		</pre>
	);
}

function ResultView({pair}: {pair: ToolCallPair}) {
	if (pair.status === 'running') {
		return (
			<div className="flex items-center gap-2 px-3 py-2 text-[11px] text-ink-faint">
				<span className="h-1.5 w-1.5 animate-pulse rounded-full bg-accent" />
				Running...
			</div>
		);
	}

	if (!pair.resultRaw) return null;

	if (pair.name === 'shell' && pair.result && typeof pair.result.exit_code === 'number') {
		const stdout = typeof pair.result.stdout === 'string' ? pair.result.stdout : '';
		const stderr = typeof pair.result.stderr === 'string' ? pair.result.stderr : '';
		const exitCode = pair.result.exit_code;

		return (
			<div className="flex flex-col">
				{exitCode !== 0 ? (
					<div className="border-app-line/20 border-b px-3 py-1.5 text-[11px] text-red-400">
						exit {exitCode}
					</div>
				) : null}
				{stdout ? (
					<pre className="max-h-48 overflow-auto whitespace-pre-wrap px-3 py-2 font-mono text-[11px] text-ink-dull">
						{stdout.replace(/\n$/, '')}
					</pre>
				) : null}
				{stderr ? (
					<pre className="max-h-40 overflow-auto whitespace-pre-wrap border-app-line/10 border-t px-3 py-2 font-mono text-[11px] text-red-300/70">
						{stderr.replace(/\n$/, '')}
					</pre>
				) : null}
			</div>
		);
	}

	return (
		<pre className="max-h-48 overflow-auto whitespace-pre-wrap px-3 py-2 font-mono text-[11px] text-ink-dull">
			{pair.resultRaw}
		</pre>
	);
}

export function ToolCall({pair}: {pair: ToolCallPair}) {
	const [expanded, setExpanded] = useState(false);
	const summary = useMemo(() => summaryForPair(pair), [pair]);

	return (
		<div
			className={clsx(
				'overflow-hidden rounded-xl border bg-app-box/35',
				pair.status === 'error' ? 'border-red-500/20' : 'border-app-line/50'
			)}
		>
			<button
				onClick={() => setExpanded((value) => !value)}
				className="flex w-full items-center gap-2 px-3 py-2 text-left"
			>
				<span className={clsx('flex shrink-0 items-center', statusColor(pair.status), pair.status === 'running' ? 'animate-pulse' : '')}>
					{statusIcon(pair.status)}
				</span>
				<span className="text-ink text-xs font-medium">{formatToolName(pair.name)}</span>
				{summary ? <span className="text-ink-faint min-w-0 flex-1 truncate text-[11px]">{summary}</span> : <span className="flex-1" />}
				<CaretDown className={clsx('text-ink-faint size-3 transition-transform', expanded ? 'rotate-180' : '')} weight="bold" />
			</button>

			{expanded ? (
				<div className="border-app-line/30 border-t">
					<ArgsView pair={pair} />
					<ResultView pair={pair} />
				</div>
			) : null}
		</div>
	);
}
