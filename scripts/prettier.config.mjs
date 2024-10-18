import config from '../prettier.config.mjs'

export default /** @type {import('prettier').Config} */ ({
	...config,
	semi: false
})
