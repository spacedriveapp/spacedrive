import * as fs from 'node:fs/promises'
import { dirname, join as joinPath, posix as path } from 'node:path'
import { env } from 'node:process'
import { setTimeout } from 'node:timers/promises'
import { fileURLToPath } from 'node:url'

const __debug = env.NODE_ENV === 'debug'
const __offline = env.OFFLINE === 'true'
const __filename = fileURLToPath(import.meta.url)
const __dirname = dirname(__filename)
const cacheDir = joinPath(__dirname, '.tmp')
await fs.mkdir(cacheDir, { recursive: true, mode: 0o751 })

// Note: Trailing slashs are important to correctly append paths
const GH = 'https://api.github.com/repos/'
const NIGTHLY = 'https://nightly.link/'

// Github routes
const RELEASES = 'releases'
const WORKFLOWS = 'actions/workflows'
const ARTIFACTS = 'actions/artifacts'

// Default GH headers
const GH_HEADERS = new Headers({
	Accept: 'application/vnd.github+json',
	'X-GitHub-Api-Version': '2022-11-28',
})

// Load github auth token if available
if ('GITHUB_TOKEN' in env && env.GITHUB_TOKEN)
	GH_HEADERS.append('Authorization', `Bearer ${env.GITHUB_TOKEN}`)

/**
 * @param {string} resource
 * @param {Headers} [headers]
 * @returns {Promise<null | {data: Buffer, header: [string, string] | undefined}>}
 */
async function getCache(resource, headers) {
	/** @type {Buffer | undefined} */
	let data
	/** @type {[string, string] | undefined} */
	let header

	// Don't cache in CI
	if (env.CI === 'true') return null

	if (headers)
		resource += Array.from(headers.entries())
			.filter(([name]) => name !== 'If-None-Match' && name !== 'If-Modified-Since')
			.flat()
			.join(':')
	try {
		const cache = JSON.parse(
			await fs.readFile(joinPath(cacheDir, Buffer.from(resource).toString('base64url')), {
				encoding: 'utf8',
			})
		)
		if (cache && typeof cache === 'object') {
			if (cache.etag && typeof cache.etag === 'string') {
				header = ['If-None-Match', cache.etag]
			} else if (cache.modifiedSince && typeof cache.modifiedSince === 'string') {
				header = ['If-Modified-Since', cache.modifiedSince]
			}

			if (cache.data && typeof cache.data === 'string')
				data = Buffer.from(cache.data, 'base64')
		}
	} catch (error) {
		if (__debug) {
			console.warn(`CACHE MISS: ${resource}`)
			console.error(error)
		}
	}

	return data ? { data, header } : null
}

/**
 * @param {Response} response
 * @param {string} resource
 * @param {Buffer} [cachedData]
 * @param {Headers} [headers]
 * @returns {Promise<Buffer>}
 */
async function setCache(response, resource, cachedData, headers) {
	const data = Buffer.from(await response.arrayBuffer())

	// Don't cache in CI
	if (env.CI === 'true') return data

	const etag = response.headers.get('ETag') || undefined
	const modifiedSince = response.headers.get('Last-Modified') || undefined
	if (headers)
		resource += Array.from(headers.entries())
			.filter(([name]) => name !== 'If-None-Match' && name !== 'If-Modified-Since')
			.flat()
			.join(':')

	if (response.status === 304 || (response.ok && data.length === 0)) {
		// Cache hit
		if (!cachedData) throw new Error('Empty cache hit ????')
		return cachedData
	}

	try {
		await fs.writeFile(
			joinPath(cacheDir, Buffer.from(resource).toString('base64url')),
			JSON.stringify({
				etag,
				modifiedSince,
				data: data.toString('base64'),
			}),
			{ mode: 0o640, flag: 'w+' }
		)
	} catch (error) {
		if (__debug) {
			console.warn(`CACHE WRITE FAIL: ${resource}`)
			console.error(error)
		}
	}

	return data
}

/**
 * @param {URL | string} resource
 * @param {Headers?} [headers]
 * @param {boolean} [preferCache]
 * @returns {Promise<Buffer>}
 */
export async function get(resource, headers, preferCache) {
	if (headers == null) headers = new Headers()
	if (resource instanceof URL) resource = resource.toString()

	const cache = await getCache(resource, headers)
	if (__offline) {
		if (cache?.data == null)
			throw new Error(`OFFLINE MODE: Cache for request ${resource} doesn't exist`)
		return cache.data
	}
	if (preferCache && cache?.data != null) return cache.data

	if (cache?.header) headers.append(...cache.header)

	const response = await fetch(resource, { headers })

	if (!response.ok) {
		if (cache?.data) {
			if (__debug) console.warn(`CACHE HIT due to fail: ${resource} ${response.statusText}`)
			return cache.data
		}
		throw new Error(response.statusText)
	}

	return await setCache(response, resource, cache?.data, headers)
}

// Header name	Description
// x-ratelimit-limit	The maximum number of requests you're permitted to make per hour.
// x-ratelimit-remaining	The number of requests remaining in the current rate limit window.
// x-ratelimit-used	The number of requests you've made in the current rate limit window.
// x-ratelimit-reset	The time at which the current rate limit window resets in UTC epoch seconds.
const RATE_LIMIT = {
	reset: 0,
	remaining: Infinity,
}

/**
 * Get resource from a Github route with some pre-defined parameters
 * @param {string} route
 * @returns {Promise<Buffer>}
 */
export async function getGh(route) {
	route = new URL(route, GH).toString()

	const cache = await getCache(route)
	if (__offline) {
		if (cache?.data == null)
			throw new Error(`OFFLINE MODE: Cache for request ${route} doesn't exist`)
		return cache?.data
	}

	if (RATE_LIMIT.remaining === 0) {
		if (cache?.data) return cache.data
		console.warn(
			`RATE LIMIT: Waiting ${RATE_LIMIT.reset} seconds before contacting Github again... [CTRL+C to cancel]`
		)
		await setTimeout(RATE_LIMIT.reset * 1000)
	}

	const headers = new Headers(GH_HEADERS)
	if (cache?.header) headers.append(...cache.header)

	const response = await fetch(route, { method: 'GET', headers })

	const rateReset = Number.parseInt(response.headers.get('x-ratelimit-reset') ?? '')
	const rateRemaining = Number.parseInt(response.headers.get('x-ratelimit-remaining') ?? '')
	if (!(Number.isNaN(rateReset) || Number.isNaN(rateRemaining))) {
		const reset = rateReset - Date.now() / 1000
		if (reset > RATE_LIMIT.reset) RATE_LIMIT.reset = reset
		if (rateRemaining < RATE_LIMIT.remaining) {
			RATE_LIMIT.remaining = rateRemaining
			if (__debug) {
				console.warn(`Github remaining requests: ${RATE_LIMIT.remaining}`)
				await setTimeout(5000)
			}
		}
	}

	if (!response.ok) {
		if (cache?.data) {
			if (__debug) console.warn(`CACHE HIT due to fail: ${route} ${response.statusText}`)
			return cache.data
		}
		if (response.status === 403 && RATE_LIMIT.remaining === 0) return await getGh(route)
		throw new Error(response.statusText)
	}

	return await setCache(response, route, cache?.data)
}

/**
 * @param {string} repo
 * @yields {{name: string, downloadUrl: string}}
 */
export async function* getGhReleasesAssets(repo) {
	let page = 0
	while (true) {
		// "${_gh_url}/protocolbuffers/protobuf/releases?page=${_page}&per_page=100"
		const releases = JSON.parse(
			(await getGh(path.join(repo, `${RELEASES}?page=${page++}&per_page=100`))).toString(
				'utf8'
			)
		)

		if (!Array.isArray(releases)) throw new Error(`Error: ${JSON.stringify(releases)}`)
		if (releases.length === 0) return

		for (const release of /** @type {unknown[]} */ (releases)) {
			if (
				!(
					release &&
					typeof release === 'object' &&
					'assets' in release &&
					Array.isArray(release.assets)
				)
			)
				throw new Error(`Invalid release: ${release}`)

			if ('prerelease' in release && release.prerelease) continue

			for (const asset of /** @type {unknown[]} */ (release.assets)) {
				if (
					!(
						asset &&
						typeof asset === 'object' &&
						'name' in asset &&
						typeof asset.name === 'string' &&
						'browser_download_url' in asset &&
						typeof asset.browser_download_url === 'string'
					)
				)
					throw new Error(`Invalid release.asset: ${asset}`)

				yield { name: asset.name, downloadUrl: asset.browser_download_url }
			}
		}
	}
}

/**
 * @param {string} repo
 * @param {string} yaml
 * @param {string | Array.<string> | Set.<string>} [branch]
 * @yields {{ id: number, name: string }}
 */
export async function* getGhWorkflowRunArtifacts(repo, yaml, branch) {
	if (!branch) branch = 'main'
	if (typeof branch === 'string') branch = [branch]
	if (!(branch instanceof Set)) branch = new Set(branch)

	let page = 0
	while (true) {
		const workflow = /** @type {unknown} */ (
			JSON.parse(
				(
					await getGh(
						path.join(
							repo,
							WORKFLOWS,
							yaml,
							`runs?page=${page++}&per_page=100&status=success`
						)
					)
				).toString('utf8')
			)
		)
		if (
			!(
				workflow &&
				typeof workflow === 'object' &&
				'workflow_runs' in workflow &&
				Array.isArray(workflow.workflow_runs)
			)
		)
			throw new Error(`Error: ${JSON.stringify(workflow)}`)

		if (workflow.workflow_runs.length === 0) return

		for (const run of /** @type {unknown[]} */ (workflow.workflow_runs)) {
			if (
				!(
					run &&
					typeof run === 'object' &&
					'head_branch' in run &&
					typeof run.head_branch === 'string' &&
					'artifacts_url' in run &&
					typeof run.artifacts_url === 'string'
				)
			)
				throw new Error(`Invalid Workflow run: ${run}`)

			if (!branch.has(run.head_branch)) continue

			const response = /** @type {unknown} */ (
				JSON.parse((await getGh(run.artifacts_url)).toString('utf8'))
			)

			if (
				!(
					response &&
					typeof response === 'object' &&
					'artifacts' in response &&
					Array.isArray(response.artifacts)
				)
			)
				throw new Error(`Error: ${JSON.stringify(response)}`)

			for (const artifact of /** @type {unknown[]} */ (response.artifacts)) {
				if (
					!(
						artifact &&
						typeof artifact === 'object' &&
						'id' in artifact &&
						typeof artifact.id === 'number' &&
						'name' in artifact &&
						typeof artifact.name === 'string'
					)
				)
					throw new Error(`Invalid artifact: ${artifact}`)

				yield { id: artifact.id, name: artifact.name }
			}
		}
	}
}

/**
 * @param {string} repo
 * @param {number} id
 * @returns {Promise<Buffer>}
 */
export async function getGhArtifactContent(repo, id) {
	// Artifacts can only be downloaded directly from Github with authorized requests
	if (GH_HEADERS.has('Authorization')) {
		try {
			// "${_gh_url}/${_sd_gh_path}/actions/artifacts/${_artifact_id}/zip"
			return await getGh(path.join(repo, ARTIFACTS, id.toString(), 'zip'))
		} catch (error) {
			if (__debug) {
				console.warn('Failed to download artifact from github, fallback to nightly.link')
				console.error(error)
			}
		}
	}

	/**
	 * nightly.link is a workaround for the lack of a public GitHub API to download artifacts from a workflow run
	 * https://github.com/actions/upload-artifact/issues/51
	 * Use it when running in evironments that are not authenticated with github
	 * "https://nightly.link/${_sd_gh_path}/actions/artifacts/${_artifact_id}.zip"
	 */
	return await get(new URL(path.join(repo, ARTIFACTS, `${id}.zip`), NIGTHLY), null, true)
}
