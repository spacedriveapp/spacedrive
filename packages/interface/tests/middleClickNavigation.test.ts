import {describe, expect, it} from 'bun:test';
import {
	installMiddleClickNavigationGuard,
	installMiddleClickNavigationGuardForPlatform,
	shouldInstallMiddleClickNavigationGuard
} from '../src/util/middleClickNavigation';

function mouseEvent(type: string, button: number) {
	const event = new Event(type, {cancelable: true});
	Object.defineProperty(event, 'button', {value: button});
	return event;
}

describe('middle-click navigation guard', () => {
	it('is enabled only for the Windows desktop app', () => {
		expect(
			shouldInstallMiddleClickNavigationGuard('tauri', 'Windows NT 10.0')
		).toBe(true);
		expect(
			shouldInstallMiddleClickNavigationGuard('web', 'Windows NT 10.0')
		).toBe(false);
		expect(
			shouldInstallMiddleClickNavigationGuard('tauri', 'Linux x86_64')
		).toBe(false);
	});

	it('prevents middle-button auxiliary clicks', () => {
		const target = new EventTarget();
		installMiddleClickNavigationGuard(target);
		const event = mouseEvent('auxclick', 1);

		target.dispatchEvent(event);

		expect(event.defaultPrevented).toBe(true);
	});

	it.each([0, 2])('does not prevent mouse button %i', (button) => {
		const target = new EventTarget();
		installMiddleClickNavigationGuard(target);
		const event = mouseEvent('auxclick', button);

		target.dispatchEvent(event);

		expect(event.defaultPrevented).toBe(false);
	});

	it('does not prevent the middle-button press that starts autoscroll', () => {
		const target = new EventTarget();
		installMiddleClickNavigationGuard(target);
		const event = mouseEvent('mousedown', 1);

		target.dispatchEvent(event);

		expect(event.defaultPrevented).toBe(false);
	});

	it('removes the guard during cleanup', () => {
		const target = new EventTarget();
		const cleanup = installMiddleClickNavigationGuard(target);
		cleanup();
		const event = mouseEvent('auxclick', 1);

		target.dispatchEvent(event);

		expect(event.defaultPrevented).toBe(false);
	});

	it('does not require browser globals when the platform provides no environment', () => {
		expect(installMiddleClickNavigationGuardForPlatform('web')).toBeUndefined();
		expect(installMiddleClickNavigationGuardForPlatform('tauri')).toBeUndefined();
	});

	it('installs and cleans up through the Tauri-provided environment', () => {
		const eventTarget = new EventTarget();
		const cleanup = installMiddleClickNavigationGuardForPlatform('tauri', {
			userAgent: 'Windows NT 10.0',
			eventTarget
		});
		const guardedEvent = mouseEvent('auxclick', 1);

		eventTarget.dispatchEvent(guardedEvent);
		expect(guardedEvent.defaultPrevented).toBe(true);

		cleanup?.();
		const eventAfterCleanup = mouseEvent('auxclick', 1);
		eventTarget.dispatchEvent(eventAfterCleanup);
		expect(eventAfterCleanup.defaultPrevented).toBe(false);
	});
});
