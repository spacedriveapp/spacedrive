/**
 * Simple function that implements a spinner animation in the terminal.
 * It receives an AbortController as argument and return a promise that resolves when the AbortController is signaled.
 * @param {AbortController} abortController
 * @returns {Promise<void>}
 */
export function spinnerAnimation(abortController) {
	const frames = ['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧', '⠇', '⠏']
	let frameIndex = 0

	return new Promise(resolve => {
		const intervalId = setInterval(() => {
			process.stdout.write(`\r${frames[frameIndex++]}`)
			frameIndex %= frames.length
		}, 100)

		const onAbort = () => {
			clearInterval(intervalId)
			process.stdout.write('\r \r') // Clear spinner
			resolve()
		}

		if (abortController.signal.aborted) {
			onAbort()
		} else {
			abortController.signal.addEventListener('abort', onAbort)
		}
	})
}

/**
 * Wrap a long running task with a spinner animation.
 * @template T
 * @param {Promise<T>} promise
 * @returns {Promise<T>}
 */
export async function spinTask(promise) {
	const spinnerControl = new AbortController()
	const [, result] = await Promise.all([
		spinnerAnimation(spinnerControl),
		promise.finally(() => spinnerControl.abort('Task is over')),
	])
	return result
}
