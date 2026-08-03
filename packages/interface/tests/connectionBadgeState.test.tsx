import {TooltipProvider} from '@spacedrive/primitives';
import {describe, expect, test} from 'bun:test';
import {renderToStaticMarkup} from 'react-dom/server';
import {ConnectionBadge} from '../src/routes/overview/ConnectionBadge';
import {getConnectionBadgeState} from '../src/routes/overview/connectionBadgeState';

describe('getConnectionBadgeState', () => {
	test('identifies the current device before checking its connection', () => {
		expect(
			getConnectionBadgeState({
				method: null,
				online: false,
				current: true
			})
		).toBe('Current');
	});

	test('identifies an offline device without a connection method', () => {
		expect(
			getConnectionBadgeState({
				method: null,
				online: false,
				current: false
			})
		).toBe('Offline');
	});

	test.each(['LocalNetwork', 'DirectInternet', 'RelayProxy'] as const)(
		'uses the known %s connection method',
		(method) => {
			expect(
				getConnectionBadgeState({method, online: true, current: false})
			).toBe(method);
		}
	);

	test('shows an online device as connecting until its method is known', () => {
		expect(
			getConnectionBadgeState({
				method: undefined,
				online: true,
				current: false
			})
		).toBe('Connecting');
	});

	test('renders the connecting state when a paired device has no method yet', () => {
		const markup = renderToStaticMarkup(
			<TooltipProvider>
				<ConnectionBadge method={null} online current={false} />
			</TooltipProvider>
		);

		expect(markup).toContain('Connecting');
	});
});
