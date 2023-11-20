import * as fs from 'node:fs/promises'
import { dirname, join as joinPath } from 'node:path'
import { env } from 'node:process'
import { fileURLToPath } from 'node:url'

import { fetch, Headers } from 'undici'

const __debug = env.NODE_ENV === 'debug'
const __offline = env.OFFLINE === 'true'
const __filename = fileURLToPath(import.meta.url)
const __dirname = dirname(__filename)
const cacheDir = joinPath(__dirname, '.tmp')
await fs.mkdir(cacheDir, { recursive: true, mode: 0o751 })

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
 * @param {import('undici').Response} response
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
