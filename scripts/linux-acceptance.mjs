// Drives the Linux-provable live-proof rows against a running sd-server and
// saves one screenshot per row. Playwright is not a workspace dependency, so
// copy this file into a scratch directory, run `npm i playwright` and
// `npx playwright install chromium` there, then:
//
//   SD_URL=http://localhost:8080 SD_DEVICE=cox-local SD_HOME=/path/to/fixture/home \
//   SD_EXTERNAL=/mnt/sd-external OUT=shots node linux-acceptance.mjs
//
// The fixture is a tracked source at SD_HOME with Videos/clip-00.mp4, and a
// tracked ext4 volume at SD_EXTERNAL with an Inbox folder of note-NNN files. The app restores
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
	const snap = (target = page) => target.screenshot({ path: `${out}/${name}.png` });
	try {
		const detail = await fn(snap);
		results.push({ row: name, ok: true, detail });
	} catch (e) {
		await page.screenshot({ path: `${out}/${name}-fail.png` }).catch(() => {});
		results.push({ row: name, ok: false, detail: String(e).slice(0, 300) });
	}
};
const expect = (value, message) => {
	if (!value) throw new Error(message);
	return value;
};
const body = async (target = page) => {
	const text = await target.innerText("body");
	expect(!/Failed to load|Something went wrong/i.test(text), `error state: ${text.slice(0, 200)}`);
	return text;
};

await page.goto(url);
await page.waitForTimeout(5000);

await row("sidebar", async (snap) => {
	const text = await body();
	for (const item of ["Places", "Storage", "Trash"])
		expect(text.toLowerCase().includes(item.toLowerCase()), `sidebar has no ${item}`);
	await snap();
	return text.slice(0, 200);
});

await row("sources", async (snap) => {
	await go("/sources");
	const text = await body();
	const volume = external.split("/").filter(Boolean).pop();
	expect(text.includes(volume), `the external volume ${volume} is not listed`);
	await snap();
	return text.match(/(unmounted|locked|offline copy)[^\n]*/gi);
});

await row("trash", async (snap) => {
	await page.click("text=Recents");
	await page.waitForTimeout(1500);
	await page.click("text=Trash");
	await page.waitForTimeout(3000);
	const match = expect(
		(await body()).match(/\d+ items? Spacedrive trashed|Nothing Spacedrive trashed/)?.[0],
		"the Trash view shows neither items nor its empty state",
	);
	await snap();
	return match;
});

if (device && home) {
	await row("quick-preview-video", async (snap) => {
		await go(explorer(`${home}/Videos`));
		await page.getByText("clip-00", { exact: false }).first().click();
		await page.keyboard.press("Space");
		await page.waitForTimeout(6000);
		const video = await page.evaluate(() => {
			const v = document.querySelector("video");
			return v && { src: v.currentSrc, readyState: v.readyState, width: v.videoWidth };
		});
		await snap();
		await page.keyboard.press("Escape");
		expect(video && video.readyState >= 2, `video did not load: ${JSON.stringify(video)}`);
		return video;
	});
}

if (device) {
	await row("external-listing", async (snap) => {
		await go(explorer(`${external}/Inbox`));
		const names = expect((await body()).match(/note-\d+[^\n]*/g), "the Inbox listing shows no files");
		await snap();
		return names.slice(0, 5);
	});
}

// SD_RAW is a folder of camera raw files: one with an embedded preview, such as
// a CR2, and one named like a raw that holds none. Quick Preview must show the
// embedded JPEG or the kind's icon for each, never a broken image.
const raw = process.env.SD_RAW;
if (device && raw) {
	await row("raw-preview", async (snap) => {
		const { readdirSync } = await import("node:fs");
		const files = readdirSync(raw).filter((name) =>
			/\.(3fr|arw|cr2|cr3|dng|nef|orf|pef|raf|rw2|srw)$/i.test(name),
		);
		expect(files.length > 0, `${raw} holds no raw files`);
		await go(explorer(raw));
		const results = [];
		for (const name of files) {
			await page.getByText(name.replace(/\.[^.]+$/, ""), { exact: false }).first().click();
			await page.keyboard.press("Space");
			await page.waitForTimeout(5000);
			const images = await page.evaluate(() =>
				[...document.querySelectorAll("img")]
					.filter((i) => i.getBoundingClientRect().width >= 200 && getComputedStyle(i).opacity !== "0")
					.map((i) => ({ ok: i.complete && i.naturalWidth > 0, width: i.naturalWidth, src: i.src.slice(-60) })),
			);
			await snap();
			await page.keyboard.press("Escape");
			await page.waitForTimeout(1000);
			expect(images.length > 0 && images.every((i) => i.ok), `${name} previews as ${JSON.stringify(images)}`);
			results.push({ name, images });
		}
		return results;
	});
}

await row("settings-tab", async (snap) => {
	const [tab] = await Promise.all([
		page.context().waitForEvent("page", { timeout: 8000 }),
		page.click("#sidebar-item-settings"),
	]);
	await tab.waitForTimeout(4000);
	try {
		expect(new URL(tab.url()).pathname.startsWith("/settings"), `opened ${tab.url()}`);
		const text = await body(tab);
		expect(/General/.test(text), "the Settings tab has no General section");
		await snap(tab);
		return { url: tab.url(), text: text.slice(0, 200) };
	} finally {
		await tab.close();
	}
});

console.log(JSON.stringify(results, null, 1));
await browser.close();
process.exit(results.every((r) => r.ok) ? 0 : 1);
