// Drives the Linux-provable live-proof rows against a running sd-server and
// saves one screenshot per row. Playwright is not a workspace dependency, so
// copy this file into a scratch directory, run `npm i playwright` and
// `npx playwright install chromium` there, then:
//
//   SD_URL=http://localhost:8080 SD_DEVICE=cox-local SD_HOME=/path/to/fixture/home \
//   SD_EXTERNAL=/mnt/sd-external OUT=shots node linux-acceptance.mjs
//
// The fixture is a tracked source at SD_HOME with Videos/clip-00.mp4, and a
// tracked ext4 volume at SD_EXTERNAL with an Inbox folder. The app restores
// its last tab on a cold load, so route changes go through history.pushState
// after the shell is up. SD_DEVICE is the device slug from sd-cli.
import { chromium } from "playwright";
import { mkdirSync } from "node:fs";

const url = process.env.SD_URL ?? "http://localhost:8080";
const device = process.env.SD_DEVICE;
const home = process.env.SD_HOME;
const external = process.env.SD_EXTERNAL ?? "/mnt/sd-external";
const out = process.env.OUT ?? "shots";
mkdirSync(out, { recursive: true });

const browser = await chromium.launch();
const page = await browser.newPage({ viewport: { width: 1600, height: 1000 } });
const results = [];

const go = async (path) => {
	await page.evaluate((p) => {
		history.pushState({}, "", p);
		dispatchEvent(new PopStateEvent("popstate"));
	}, path);
	await page.waitForTimeout(4000);
};
const explorer = (path) =>
	`/explorer?path=${encodeURIComponent(JSON.stringify({ Physical: { device_slug: device, path } }))}`;
const row = async (name, fn) => {
	try {
		const detail = await fn();
		await page.screenshot({ path: `${out}/${name}.png` });
		results.push({ row: name, ok: true, detail });
	} catch (e) {
		await page.screenshot({ path: `${out}/${name}-fail.png` }).catch(() => {});
		results.push({ row: name, ok: false, detail: String(e).slice(0, 300) });
	}
};

await page.goto(url);
await page.waitForTimeout(5000);

await row("sidebar", async () => (await page.innerText("body")).slice(0, 200));

await row("sources-badges", async () => {
	await go("/sources");
	return (await page.innerText("body")).match(/(unmounted|locked|offline copy)[^\n]*/gi);
});

await row("trash", async () => {
	await page.click("text=Recents");
	await page.waitForTimeout(1500);
	await page.click("text=Trash");
	await page.waitForTimeout(3000);
	return (await page.innerText("body")).match(/\d+ items? Spacedrive trashed|Nothing Spacedrive trashed/)?.[0];
});

if (device && home) {
	await row("quick-preview-video", async () => {
		await go(explorer(`${home}/Videos`));
		await page.getByText("clip-00", { exact: false }).first().click();
		await page.keyboard.press("Space");
		await page.waitForTimeout(6000);
		const video = await page.evaluate(() => {
			const v = document.querySelector("video");
			return v && { src: v.currentSrc, readyState: v.readyState, width: v.videoWidth };
		});
		await page.keyboard.press("Escape");
		if (!video || video.readyState < 2) throw new Error(`video did not load: ${JSON.stringify(video)}`);
		return video;
	});
}

if (device) {
	await row("external-listing", async () => {
		await go(explorer(`${external}/Inbox`));
		return (await page.innerText("body")).slice(0, 300);
	});
}

await row("settings-tab", async () => {
	const [tab] = await Promise.all([
		page.context().waitForEvent("page", { timeout: 8000 }),
		page.click("#sidebar-item-settings"),
	]);
	await tab.waitForTimeout(4000);
	const text = (await tab.innerText("body")).slice(0, 200);
	await tab.close();
	return { url: tab.url(), text };
});

console.log(JSON.stringify(results, null, 1));
await browser.close();
process.exit(results.every((r) => r.ok) ? 0 : 1);
