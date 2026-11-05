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

// Every Media view cell is a hot-tier tile. SD_EXTERNAL needs a few images for
// this row. Point SD_URL at an sd-server that reused a daemon another client
// started with a different --data-dir to cover that case too.
if (device) {
	await row("media-view-thumbnails", async (snap) => {
		const tiles = new Map();
		const record = (response) => {
			const [, tile] = response.url().split("/hot-thumb/");
			if (tile) tiles.set(tile.split("/").slice(0, 2).join("/"), response.status());
		};
		page.on("response", record);
		await go(explorer(external));
		await page.getByText("Views").first().click();
		await page.getByText("Media", { exact: true }).first().click();
		await page.waitForTimeout(10000);
		page.off("response", record);
		const broken = await page.evaluate(
			() =>
				[...document.querySelectorAll("img")].filter(
					(i) => i.src.includes("/hot-thumb/") && !(i.complete && i.naturalWidth > 0),
				).length,
		);
		await snap();
		await page.getByText("Views").first().click();
		await page.getByText("Grid", { exact: true }).first().click();
		const failed = [...tiles.values()].filter((status) => status !== 200).length;
		expect(tiles.size > 0, "the Media view requested no tiles");
		expect(failed === 0, `${failed} of ${tiles.size} tiles did not load`);
		expect(broken === 0, `${broken} cells show a broken image`);
		return { tiles: tiles.size };
	});
}

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
	await row("video-thumbstrip", async (snap) => {
		await go(explorer(`${home}/Videos`));
		const box = expect(
			await page.locator("img[src*='/hot-thumb/']").first().boundingBox(),
			"no video poster to hover",
		);
		const strip = page.waitForResponse((r) => r.url().includes("/hot-thumbstrip/"), {
			timeout: 20000,
		});
		for (let step = 0; step <= 10; step++) {
			await page.mouse.move(box.x + 2 + ((box.width - 4) * step) / 10, box.y + box.height / 2);
			await page.waitForTimeout(150);
		}
		const response = await strip;
		await page.waitForTimeout(500);
		await snap();
		await page.mouse.move(0, 0);
		expect(response.status() === 200, `the thumbstrip answered ${response.status()}`);
		return response.url().split("/hot-thumbstrip/")[1];
	});
}

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

// SD_CLI is the sd-cli command, with its --data-dir, for the daemon behind SD_URL.
const cli = process.env.SD_CLI;
if (cli && home) {
	await row("cli-index-start", async () => {
		const { execSync } = await import("node:child_process");
		// The path goes through the environment so the shell never parses it.
		const out = execSync(`${cli} --format json index start "$SD_HOME"`, {
			encoding: "utf8",
			env: { ...process.env, SD_HOME: home },
		});
		const receipt = JSON.parse(out);
		expect(/^[0-9a-f-]{36}$/.test(receipt.id ?? ""), `no job id in ${out}`);
		return receipt;
	});
}

console.log(JSON.stringify(results, null, 1));
await browser.close();
process.exit(results.every((r) => r.ok) ? 0 : 1);
