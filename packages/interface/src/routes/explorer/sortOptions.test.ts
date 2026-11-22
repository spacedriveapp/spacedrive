import { describe, expect, test } from "bun:test";
import { coerceSortBy, searchSortField, sortOptionsFor } from "./sortOptions";

describe("sort menu state", () => {
	test("every view offers date taken", () => {
		for (const view of ["grid", "list", "column", "media"] as const) {
			expect(sortOptionsFor(view).map((o) => o.value)).toContain("datetaken");
		}
	});

	test("a folder view keeps type and the media view keeps created", () => {
		expect(sortOptionsFor("grid").map((o) => o.value)).toEqual([
			"name",
			"modified",
			"datetaken",
			"size",
			"type",
		]);
		expect(sortOptionsFor("media").map((o) => o.value)).toEqual([
			"datetaken",
			"modified",
			"created",
			"name",
			"size",
		]);
	});

	test("date taken survives a view change in both directions", () => {
		expect(coerceSortBy("media", "datetaken")).toBe("datetaken");
		expect(coerceSortBy("grid", "datetaken")).toBe("datetaken");
		expect(coerceSortBy("list", "datetaken")).toBe("datetaken");
	});

	test("an order a view does not offer falls back to the view's first", () => {
		expect(coerceSortBy("media", "type")).toBe("datetaken");
		expect(coerceSortBy("grid", "created")).toBe("name");
		expect(coerceSortBy("column", undefined)).toBe("name");
	});

	test("search sorts by capture time for date taken", () => {
		expect(searchSortField("datetaken")).toBe("CapturedAt");
		expect(searchSortField("modified")).toBe("ModifiedAt");
		expect(searchSortField("type")).toBe("Relevance");
		expect(searchSortField(null)).toBe("Relevance");
	});
});
