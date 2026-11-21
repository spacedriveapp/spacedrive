// Videos library view. A plain ES module: the client imports it and calls
// mount(el, ctx), so it carries no framework and depends on no app version.

const STYLE = `
.sdv { display: flex; flex-direction: column; height: 100%; font-size: 13px; color: inherit; }
.sdv-tabs { display: flex; gap: 4px; padding: 10px 14px; border-bottom: 1px solid rgba(128,128,128,.2); }
.sdv-tab { background: none; border: 0; color: inherit; opacity: .6; padding: 5px 10px; border-radius: 6px; cursor: pointer; font: inherit; }
.sdv-tab[aria-selected="true"] { opacity: 1; background: rgba(128,128,128,.18); }
.sdv-body { flex: 1; min-height: 0; display: flex; }
.sdv-list { width: 260px; overflow: auto; border-right: 1px solid rgba(128,128,128,.2); }
.sdv-item { display: block; width: 100%; text-align: left; background: none; border: 0; color: inherit; font: inherit; padding: 8px 14px; cursor: pointer; }
.sdv-item:hover { background: rgba(128,128,128,.1); }
.sdv-item[aria-selected="true"] { background: rgba(128,128,128,.2); }
.sdv-sub { opacity: .55; font-size: 11px; }
.sdv-detail { flex: 1; overflow: auto; padding: 14px 18px; }
.sdv-h { font-size: 15px; font-weight: 600; margin: 0 0 4px; }
.sdv-note { opacity: .6; margin: 6px 0 12px; }
.sdv-video { width: 100%; max-height: 300px; background: #000; border-radius: 8px; margin-bottom: 12px; }
.sdv-shot { display: grid; grid-template-columns: 160px 1fr; gap: 12px; padding: 10px 0; border-bottom: 1px solid rgba(128,128,128,.15); }
.sdv-thumb { width: 160px; aspect-ratio: 16/9; object-fit: cover; border-radius: 6px; background: rgba(128,128,128,.2); cursor: pointer; }
.sdv-time { font-variant-numeric: tabular-nums; opacity: .6; font-size: 11px; }
.sdv-desc { margin: 2px 0 6px; }
.sdv-chip { display: inline-block; padding: 1px 8px; margin: 0 4px 4px 0; border-radius: 10px; background: rgba(128,128,128,.18); font-size: 11px; border: 0; color: inherit; cursor: pointer; font-family: inherit; }
.sdv-tx { opacity: .75; font-size: 12px; margin-top: 4px; white-space: pre-wrap; }
.sdv-grid { display: grid; grid-template-columns: repeat(auto-fill, minmax(150px, 1fr)); gap: 14px; }
.sdv-card { background: none; border: 0; color: inherit; font: inherit; text-align: left; cursor: pointer; padding: 0; }
.sdv-card img { width: 100%; aspect-ratio: 1; object-fit: cover; border-radius: 10px; background: rgba(128,128,128,.2); }
.sdv-row { display: flex; gap: 8px; align-items: center; margin: 8px 0 14px; flex-wrap: wrap; }
.sdv-row input, .sdv-row select { background: rgba(128,128,128,.12); border: 1px solid rgba(128,128,128,.3); border-radius: 6px; color: inherit; padding: 5px 8px; font: inherit; }
.sdv-btn { background: rgba(128,128,128,.2); border: 0; border-radius: 6px; color: inherit; padding: 5px 10px; cursor: pointer; font: inherit; }
.sdv-empty { opacity: .6; padding: 40px; text-align: center; }
`;

const fmt = (ms) => {
	const s = Math.floor(ms / 1000);
	const pad = (n) => String(n).padStart(2, "0");
	return `${pad(Math.floor(s / 3600))}:${pad(Math.floor(s / 60) % 60)}:${pad(s % 60)}`;
};

const h = (tag, props = {}, ...children) => {
	const el = document.createElement(tag);
	for (const [k, v] of Object.entries(props)) {
		if (v == null || v === false) continue;
		if (k.startsWith("on")) el.addEventListener(k.slice(2).toLowerCase(), v);
		else if (k === "class") el.className = v;
		else el.setAttribute(k, v === true ? "" : v);
	}
	for (const child of children.flat()) {
		if (child == null || child === false) continue;
		el.append(child instanceof Node ? child : document.createTextNode(String(child)));
	}
	return el;
};

// Transcript segments in whatever unit the core wrote them: milliseconds
// when the field says so, seconds otherwise.
const segmentsOf = (doc) => {
	const list = Array.isArray(doc) ? doc : doc?.segments ?? [];
	return list.map((s) => ({
		start: s.start_ms ?? Math.round((s.start ?? 0) * 1000),
		end: s.end_ms ?? Math.round((s.end ?? 0) * 1000),
		text: (s.text ?? "").trim(),
	}));
};

const UNAVAILABLE = {
	image_caption: "no image caption model is installed",
	face_detection: "no face detection model is installed",
	scene_classification: "no scene model is installed",
};

export function mount(el, ctx) {
	const state = { tab: "footage", footage: [], people: [], file: null, person: null, place: null, tag: null, transcripts: new Map(), error: null };
	const root = h("div", { class: "sdv" });
	el.replaceChildren(h("style", {}, STYLE), root);
	let disposed = false;

	const keyframeUrl = (file, shot) => ctx.buildSidecarUrl(file.id, "keyframe", shot.keyframe, "jpg");
	const personName = (id) => {
		const p = state.people.find((p) => p.id === id);
		return p?.name || "Unnamed";
	};
	const visiblePeople = () => state.people.filter((p) => !p.hidden && p.faces.length > 0);

	const load = async () => {
		try {
			const [footage, people] = await Promise.all([ctx.readModels("Footage"), ctx.readModels("Person")]);
			state.footage = footage.sort((a, b) => a.name.localeCompare(b.name));
			state.people = people;
			state.error = null;
		} catch (error) {
			state.error = String(error?.message ?? error);
		}
		if (!disposed) render();
	};

	const transcript = (file) => {
		if (state.transcripts.has(file.id)) return state.transcripts.get(file.id);
		state.transcripts.set(file.id, null);
		const url = ctx.buildSidecarUrl(file.id, "transcript", "default", "json");
		if (url) {
			fetch(url)
				.then((r) => (r.ok ? r.json() : null))
				.then((doc) => {
					state.transcripts.set(file.id, doc ? segmentsOf(doc) : []);
					if (!disposed) render();
				})
				.catch(() => state.transcripts.set(file.id, []));
		}
		return null;
	};

	const edit = async (action, fields) => {
		await ctx.runJob("edit_roster", { action, ...fields });
		// The edit runs as a job; reread once it has had time to land.
		setTimeout(load, 1200);
	};

	const shotRow = (file, shot, { video, segments, person } = {}) =>
		h("div", { class: "sdv-shot" },
			h("img", {
				class: "sdv-thumb", src: keyframeUrl(file, shot) ?? undefined, alt: "", loading: "lazy",
				onClick: () => {
					if (video) { video.currentTime = shot.start_ms / 1000; video.play().catch(() => {}); }
					else { state.tab = "footage"; state.file = file.id; render(); }
				},
			}),
			h("div", {},
				h("div", { class: "sdv-time" }, `${fmt(shot.start_ms)} to ${fmt(shot.end_ms)}`, person ? ` · ${file.name}` : ""),
				h("div", { class: "sdv-desc" }, shot.description ?? h("span", { class: "sdv-sub" }, "No description yet")),
				shot.places.map((p) => h("button", { class: "sdv-chip", onClick: () => { state.tab = "places"; state.place = p; render(); } }, p)),
				shot.people.map((id) => h("button", { class: "sdv-chip", onClick: () => { state.tab = "people"; state.person = id; render(); } }, personName(id))),
				person ? faceActions(file, shot, person) : null,
				segments ? h("div", { class: "sdv-tx" }, segments.filter((s) => s.end > shot.start_ms && s.start < shot.end_ms).map((s) => s.text).join(" ")) : null,
			),
		);

	const faceActions = (file, shot, person) => {
		const faces = person.faces.filter((f) => f.media_uuid === file.id && f.shot_id === shot.id);
		return faces.map((face) => h("button", { class: "sdv-chip", title: "Take this face off the person", onClick: () => edit("exclude", { person: person.id, face }) }, "Not them"));
	};

	const footageTab = () => {
		if (state.footage.length === 0) return h("div", { class: "sdv-empty" }, "No footage analyzed yet. Run the Videos analyze job on a library with video.");
		const file = state.footage.find((f) => f.id === state.file) ?? state.footage[0];
		state.file = file.id;
		const segments = transcript(file);
		const proxy = ctx.buildSidecarUrl(file.id, "proxy", "720p", "mp4");
		const video = proxy ? h("video", { class: "sdv-video", src: proxy, controls: true, preload: "metadata" }) : null;
		const notes = [];
		if (file.status === "waiting_for_shots") notes.push("Waiting for the core to split this video into shots.");
		for (const c of file.unavailable) notes.push(`${c} not available: ${UNAVAILABLE[c] ?? "no provider"}.`);
		return [
			h("div", { class: "sdv-list" }, state.footage.map((f) =>
				h("button", { class: "sdv-item", "aria-selected": f.id === file.id ? "true" : "false", onClick: () => { state.file = f.id; render(); } },
					h("div", {}, f.name), h("div", { class: "sdv-sub" }, f.status === "analyzed" ? `${f.shots.length} shots` : "waiting for shots")))),
			h("div", { class: "sdv-detail" },
				h("div", { class: "sdv-h" }, file.name),
				h("div", { class: "sdv-sub" }, file.relative_path),
				notes.map((n) => h("div", { class: "sdv-note" }, n)),
				video,
				file.shots.map((shot) => shotRow(file, shot, { video, segments: segments ?? undefined })),
			),
		];
	};

	const shotsWhere = (test) => state.footage.flatMap((file) => file.shots.filter((s) => test(file, s)).map((shot) => ({ file, shot })));

	const peopleTab = () => {
		const people = visiblePeople();
		const person = people.find((p) => p.id === state.person);
		if (!person) {
			if (people.length === 0) return h("div", { class: "sdv-empty" }, "No people yet. Faces appear once a face detection model is installed and the analyze job runs.");
			return h("div", { class: "sdv-detail" }, h("div", { class: "sdv-grid" }, people.map((p) => {
				const file = state.footage.find((f) => f.id === p.cover?.media_uuid);
				const shot = file?.shots.find((s) => s.id === p.cover?.shot_id);
				return h("button", { class: "sdv-card", onClick: () => { state.person = p.id; render(); } },
					h("img", { src: file && shot ? keyframeUrl(file, shot) ?? undefined : undefined, alt: "" }),
					h("div", {}, p.name || "Unnamed"), h("div", { class: "sdv-sub" }, `${p.shot_count} ${p.shot_count === 1 ? "shot" : "shots"} in ${p.file_count} ${p.file_count === 1 ? "file" : "files"}`));
			})));
		}
		const name = h("input", { value: person.name ?? "", placeholder: "Name this person" });
		const others = people.filter((p) => p.id !== person.id);
		const merge = h("select", {}, h("option", { value: "" }, "Merge into..."), others.map((p) => h("option", { value: p.id }, p.name || `Unnamed (${p.shot_count} shots)`)));
		return h("div", { class: "sdv-detail" },
			h("button", { class: "sdv-btn", onClick: () => { state.person = null; render(); } }, "All people"),
			h("div", { class: "sdv-row" },
				name, h("button", { class: "sdv-btn", onClick: () => edit("name", { person: person.id, name: name.value }) }, "Save name"),
				h("button", { class: "sdv-btn", onClick: () => edit("hide", { person: person.id }) }, "Hide"),
				merge, h("button", { class: "sdv-btn", onClick: () => merge.value && edit("merge", { person: person.id, into: merge.value }) }, "Merge")),
			shotsWhere((_, s) => s.people.includes(person.id)).map(({ file, shot }) => shotRow(file, shot, { person })),
		);
	};

	const groupTab = (labelsOf, selected, select, empty) => {
		const groups = new Map();
		for (const file of state.footage) for (const shot of file.shots) for (const label of labelsOf(file, shot)) {
			if (!groups.has(label)) groups.set(label, []);
			groups.get(label).push({ file, shot });
		}
		if (groups.size === 0) return h("div", { class: "sdv-empty" }, empty);
		const current = groups.has(selected) ? selected : [...groups.keys()].sort()[0];
		return [
			h("div", { class: "sdv-list" }, [...groups.keys()].sort().map((label) =>
				h("button", { class: "sdv-item", "aria-selected": label === current ? "true" : "false", onClick: () => select(label) },
					h("div", {}, label), h("div", { class: "sdv-sub" }, `${groups.get(label).length} shots`)))),
			h("div", { class: "sdv-detail" }, groups.get(current).map(({ file, shot }) => shotRow(file, shot, { person: null }))),
		];
	};

	const tabs = [
		["footage", "Footage", footageTab],
		["people", "People", peopleTab],
		["places", "Places", () => groupTab((_, s) => s.places, state.place, (l) => { state.place = l; render(); }, "No places yet. Places come from the scene model.")],
		["tags", "Tags", () => groupTab((_, s) => [...s.places.map((p) => `Videos/Places/${p}`), ...s.people.filter((id) => state.people.find((p) => p.id === id && p.tag)).map((id) => state.people.find((p) => p.id === id).tag)], state.tag, (l) => { state.tag = l; render(); }, "No tags yet. Naming a person or finding a place tags the files.")],
	];

	const render = () => {
		const body = state.error ? h("div", { class: "sdv-empty" }, state.error) : tabs.find(([id]) => id === state.tab)[2]();
		root.replaceChildren(
			h("div", { class: "sdv-tabs", role: "tablist" },
				tabs.map(([id, label]) => h("button", { class: "sdv-tab", role: "tab", "aria-selected": id === state.tab ? "true" : "false", onClick: () => { state.tab = id; render(); } }, label)),
				h("div", { style: "flex:1" }),
				h("button", { class: "sdv-tab", onClick: () => ctx.runJob("analyze", {}).then(() => setTimeout(load, 2000)) }, "Analyze footage"),
				h("button", { class: "sdv-tab", onClick: load }, "Refresh")),
			h("div", { class: "sdv-body" }, body),
		);
	};

	render();
	load();
	return () => { disposed = true; };
}
