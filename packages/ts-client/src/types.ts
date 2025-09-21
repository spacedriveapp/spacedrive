// To parse this data:
//
//   import { Convert, Types } from "./file";
//
//   const types = Convert.toTypes(json);
//
// These functions will throw an error if the JSON doesn't
// match the expected interface, even if the JSON is valid.

export interface Types {
    actions: Action[];
    events:  boolean;
    queries: Query[];
    types:   TypesClass;
}

export interface Action {
    input:  boolean;
    method: string;
    output: boolean;
    type:   ActionType;
}

export enum ActionType {
    CoreAction = "core_action",
    LibraryAction = "library_action",
}

export interface Query {
    input:  null;
    method: string;
    output: boolean | OutputClass;
    type:   QueryType;
}

export interface OutputClass {
    properties: OutputProperties;
    required:   string[];
    title:      string;
    type:       string;
}

export interface OutputProperties {
    built_at:      Destination;
    device_info:   Status;
    libraries:     Paths;
    library_count: Count;
    network:       Status;
    services:      Status;
    system:        Status;
    version:       Destination;
}

export interface Destination {
    type: string;
}

export interface Status {
    $ref: string;
}

export interface Paths {
    items: Status;
    type:  string;
}

export interface Count {
    format:  string;
    minimum: number;
    type:    string;
}

export enum QueryType {
    Query = "query",
}

export interface TypesClass {
    FileCopyActionOutput: FileCopyActionOutput;
    JobInfoOutput:        JobInfoOutput;
    LocationAddOutput:    LocationAddOutput;
    SdPath:               TypesSDPath;
    SdPathBatch:          SDPathBatch;
}

export interface FileCopyActionOutput {
    $schema:     string;
    description: string;
    properties:  FileCopyActionOutputProperties;
    required:    string[];
    title:       string;
    type:        string;
}

export interface FileCopyActionOutputProperties {
    destination:   Destination;
    job_id:        JobID;
    sources_count: Count;
}

export interface JobID {
    format: string;
    type:   string;
}

export interface JobInfoOutput {
    $schema:     string;
    definitions: JobInfoOutputDefinitions;
    properties:  JobInfoOutputProperties;
    required:    string[];
    title:       string;
    type:        string;
}

export interface JobInfoOutputDefinitions {
    JobStatus: JobStatus;
}

export interface JobStatus {
    description: string;
    oneOf:       JobStatusOneOf[];
}

export interface JobStatusOneOf {
    description: string;
    enum:        string[];
    type:        string;
}

export interface JobInfoOutputProperties {
    completed_at:  CompletedAt;
    error_message: ErrorMessage;
    id:            JobID;
    name:          Destination;
    progress:      JobID;
    started_at:    JobID;
    status:        Status;
}

export interface CompletedAt {
    format: string;
    type:   string[];
}

export interface ErrorMessage {
    type: string[];
}

export interface LocationAddOutput {
    $schema:     string;
    description: string;
    properties:  LocationAddOutputProperties;
    required:    string[];
    title:       string;
    type:        string;
}

export interface LocationAddOutputProperties {
    job_id:      CompletedAt;
    location_id: JobID;
    name:        ErrorMessage;
    path:        Destination;
}

export interface TypesSDPath {
    $schema:     string;
    description: string;
    oneOf:       SDPathOneOf[];
    title:       string;
}

export interface SDPathOneOf {
    additionalProperties: boolean;
    description:          string;
    properties:           OneOfProperties;
    required:             string[];
    type:                 string;
}

export interface OneOfProperties {
    Physical?: Physical;
    Content?:  Content;
}

export interface Content {
    properties: ContentProperties;
    required:   string[];
    type:       string;
}

export interface ContentProperties {
    content_id: ID;
}

export interface ID {
    description: string;
    format:      string;
    type:        string;
}

export interface Physical {
    properties: PhysicalProperties;
    required:   string[];
    type:       string;
}

export interface PhysicalProperties {
    device_id: ID;
    path:      Path;
}

export interface Path {
    description: string;
    type:        string;
}

export interface SDPathBatch {
    $schema:     string;
    definitions: SDPathBatchDefinitions;
    description: string;
    properties:  SDPathBatchProperties;
    required:    string[];
    title:       string;
    type:        string;
}

export interface SDPathBatchDefinitions {
    SdPath: DefinitionsSDPath;
}

export interface DefinitionsSDPath {
    description: string;
    oneOf:       SDPathOneOf[];
}

export interface SDPathBatchProperties {
    paths: Paths;
}

// Converts JSON strings to/from your types
// and asserts the results of JSON.parse at runtime
export class Convert {
    public static toTypes(json: string): Types {
        return cast(JSON.parse(json), r("Types"));
    }

    public static typesToJson(value: Types): string {
        return JSON.stringify(uncast(value, r("Types")), null, 2);
    }
}

function invalidValue(typ: any, val: any, key: any, parent: any = ''): never {
    const prettyTyp = prettyTypeName(typ);
    const parentText = parent ? ` on ${parent}` : '';
    const keyText = key ? ` for key "${key}"` : '';
    throw Error(`Invalid value${keyText}${parentText}. Expected ${prettyTyp} but got ${JSON.stringify(val)}`);
}

function prettyTypeName(typ: any): string {
    if (Array.isArray(typ)) {
        if (typ.length === 2 && typ[0] === undefined) {
            return `an optional ${prettyTypeName(typ[1])}`;
        } else {
            return `one of [${typ.map(a => { return prettyTypeName(a); }).join(", ")}]`;
        }
    } else if (typeof typ === "object" && typ.literal !== undefined) {
        return typ.literal;
    } else {
        return typeof typ;
    }
}

function jsonToJSProps(typ: any): any {
    if (typ.jsonToJS === undefined) {
        const map: any = {};
        typ.props.forEach((p: any) => map[p.json] = { key: p.js, typ: p.typ });
        typ.jsonToJS = map;
    }
    return typ.jsonToJS;
}

function jsToJSONProps(typ: any): any {
    if (typ.jsToJSON === undefined) {
        const map: any = {};
        typ.props.forEach((p: any) => map[p.js] = { key: p.json, typ: p.typ });
        typ.jsToJSON = map;
    }
    return typ.jsToJSON;
}

function transform(val: any, typ: any, getProps: any, key: any = '', parent: any = ''): any {
    function transformPrimitive(typ: string, val: any): any {
        if (typeof typ === typeof val) return val;
        return invalidValue(typ, val, key, parent);
    }

    function transformUnion(typs: any[], val: any): any {
        // val must validate against one typ in typs
        const l = typs.length;
        for (let i = 0; i < l; i++) {
            const typ = typs[i];
            try {
                return transform(val, typ, getProps);
            } catch (_) {}
        }
        return invalidValue(typs, val, key, parent);
    }

    function transformEnum(cases: string[], val: any): any {
        if (cases.indexOf(val) !== -1) return val;
        return invalidValue(cases.map(a => { return l(a); }), val, key, parent);
    }

    function transformArray(typ: any, val: any): any {
        // val must be an array with no invalid elements
        if (!Array.isArray(val)) return invalidValue(l("array"), val, key, parent);
        return val.map(el => transform(el, typ, getProps));
    }

    function transformDate(val: any): any {
        if (val === null) {
            return null;
        }
        const d = new Date(val);
        if (isNaN(d.valueOf())) {
            return invalidValue(l("Date"), val, key, parent);
        }
        return d;
    }

    function transformObject(props: { [k: string]: any }, additional: any, val: any): any {
        if (val === null || typeof val !== "object" || Array.isArray(val)) {
            return invalidValue(l(ref || "object"), val, key, parent);
        }
        const result: any = {};
        Object.getOwnPropertyNames(props).forEach(key => {
            const prop = props[key];
            const v = Object.prototype.hasOwnProperty.call(val, key) ? val[key] : undefined;
            result[prop.key] = transform(v, prop.typ, getProps, key, ref);
        });
        Object.getOwnPropertyNames(val).forEach(key => {
            if (!Object.prototype.hasOwnProperty.call(props, key)) {
                result[key] = transform(val[key], additional, getProps, key, ref);
            }
        });
        return result;
    }

    if (typ === "any") return val;
    if (typ === null) {
        if (val === null) return val;
        return invalidValue(typ, val, key, parent);
    }
    if (typ === false) return invalidValue(typ, val, key, parent);
    let ref: any = undefined;
    while (typeof typ === "object" && typ.ref !== undefined) {
        ref = typ.ref;
        typ = typeMap[typ.ref];
    }
    if (Array.isArray(typ)) return transformEnum(typ, val);
    if (typeof typ === "object") {
        return typ.hasOwnProperty("unionMembers") ? transformUnion(typ.unionMembers, val)
            : typ.hasOwnProperty("arrayItems")    ? transformArray(typ.arrayItems, val)
            : typ.hasOwnProperty("props")         ? transformObject(getProps(typ), typ.additional, val)
            : invalidValue(typ, val, key, parent);
    }
    // Numbers can be parsed by Date but shouldn't be.
    if (typ === Date && typeof val !== "number") return transformDate(val);
    return transformPrimitive(typ, val);
}

function cast<T>(val: any, typ: any): T {
    return transform(val, typ, jsonToJSProps);
}

function uncast<T>(val: T, typ: any): any {
    return transform(val, typ, jsToJSONProps);
}

function l(typ: any) {
    return { literal: typ };
}

function a(typ: any) {
    return { arrayItems: typ };
}

function u(...typs: any[]) {
    return { unionMembers: typs };
}

function o(props: any[], additional: any) {
    return { props, additional };
}

function m(additional: any) {
    return { props: [], additional };
}

function r(name: string) {
    return { ref: name };
}

const typeMap: any = {
    "Types": o([
        { json: "actions", js: "actions", typ: a(r("Action")) },
        { json: "events", js: "events", typ: true },
        { json: "queries", js: "queries", typ: a(r("Query")) },
        { json: "types", js: "types", typ: r("TypesClass") },
    ], false),
    "Action": o([
        { json: "input", js: "input", typ: true },
        { json: "method", js: "method", typ: "" },
        { json: "output", js: "output", typ: true },
        { json: "type", js: "type", typ: r("ActionType") },
    ], false),
    "Query": o([
        { json: "input", js: "input", typ: null },
        { json: "method", js: "method", typ: "" },
        { json: "output", js: "output", typ: u(true, r("OutputClass")) },
        { json: "type", js: "type", typ: r("QueryType") },
    ], false),
    "OutputClass": o([
        { json: "properties", js: "properties", typ: r("OutputProperties") },
        { json: "required", js: "required", typ: a("") },
        { json: "title", js: "title", typ: "" },
        { json: "type", js: "type", typ: "" },
    ], false),
    "OutputProperties": o([
        { json: "built_at", js: "built_at", typ: r("Destination") },
        { json: "device_info", js: "device_info", typ: r("Status") },
        { json: "libraries", js: "libraries", typ: r("Paths") },
        { json: "library_count", js: "library_count", typ: r("Count") },
        { json: "network", js: "network", typ: r("Status") },
        { json: "services", js: "services", typ: r("Status") },
        { json: "system", js: "system", typ: r("Status") },
        { json: "version", js: "version", typ: r("Destination") },
    ], false),
    "Destination": o([
        { json: "type", js: "type", typ: "" },
    ], false),
    "Status": o([
        { json: "$ref", js: "$ref", typ: "" },
    ], false),
    "Paths": o([
        { json: "items", js: "items", typ: r("Status") },
        { json: "type", js: "type", typ: "" },
    ], false),
    "Count": o([
        { json: "format", js: "format", typ: "" },
        { json: "minimum", js: "minimum", typ: 3.14 },
        { json: "type", js: "type", typ: "" },
    ], false),
    "TypesClass": o([
        { json: "FileCopyActionOutput", js: "FileCopyActionOutput", typ: r("FileCopyActionOutput") },
        { json: "JobInfoOutput", js: "JobInfoOutput", typ: r("JobInfoOutput") },
        { json: "LocationAddOutput", js: "LocationAddOutput", typ: r("LocationAddOutput") },
        { json: "SdPath", js: "SdPath", typ: r("TypesSDPath") },
        { json: "SdPathBatch", js: "SdPathBatch", typ: r("SDPathBatch") },
    ], false),
    "FileCopyActionOutput": o([
        { json: "$schema", js: "$schema", typ: "" },
        { json: "description", js: "description", typ: "" },
        { json: "properties", js: "properties", typ: r("FileCopyActionOutputProperties") },
        { json: "required", js: "required", typ: a("") },
        { json: "title", js: "title", typ: "" },
        { json: "type", js: "type", typ: "" },
    ], false),
    "FileCopyActionOutputProperties": o([
        { json: "destination", js: "destination", typ: r("Destination") },
        { json: "job_id", js: "job_id", typ: r("JobID") },
        { json: "sources_count", js: "sources_count", typ: r("Count") },
    ], false),
    "JobID": o([
        { json: "format", js: "format", typ: "" },
        { json: "type", js: "type", typ: "" },
    ], false),
    "JobInfoOutput": o([
        { json: "$schema", js: "$schema", typ: "" },
        { json: "definitions", js: "definitions", typ: r("JobInfoOutputDefinitions") },
        { json: "properties", js: "properties", typ: r("JobInfoOutputProperties") },
        { json: "required", js: "required", typ: a("") },
        { json: "title", js: "title", typ: "" },
        { json: "type", js: "type", typ: "" },
    ], false),
    "JobInfoOutputDefinitions": o([
        { json: "JobStatus", js: "JobStatus", typ: r("JobStatus") },
    ], false),
    "JobStatus": o([
        { json: "description", js: "description", typ: "" },
        { json: "oneOf", js: "oneOf", typ: a(r("JobStatusOneOf")) },
    ], false),
    "JobStatusOneOf": o([
        { json: "description", js: "description", typ: "" },
        { json: "enum", js: "enum", typ: a("") },
        { json: "type", js: "type", typ: "" },
    ], false),
    "JobInfoOutputProperties": o([
        { json: "completed_at", js: "completed_at", typ: r("CompletedAt") },
        { json: "error_message", js: "error_message", typ: r("ErrorMessage") },
        { json: "id", js: "id", typ: r("JobID") },
        { json: "name", js: "name", typ: r("Destination") },
        { json: "progress", js: "progress", typ: r("JobID") },
        { json: "started_at", js: "started_at", typ: r("JobID") },
        { json: "status", js: "status", typ: r("Status") },
    ], false),
    "CompletedAt": o([
        { json: "format", js: "format", typ: "" },
        { json: "type", js: "type", typ: a("") },
    ], false),
    "ErrorMessage": o([
        { json: "type", js: "type", typ: a("") },
    ], false),
    "LocationAddOutput": o([
        { json: "$schema", js: "$schema", typ: "" },
        { json: "description", js: "description", typ: "" },
        { json: "properties", js: "properties", typ: r("LocationAddOutputProperties") },
        { json: "required", js: "required", typ: a("") },
        { json: "title", js: "title", typ: "" },
        { json: "type", js: "type", typ: "" },
    ], false),
    "LocationAddOutputProperties": o([
        { json: "job_id", js: "job_id", typ: r("CompletedAt") },
        { json: "location_id", js: "location_id", typ: r("JobID") },
        { json: "name", js: "name", typ: r("ErrorMessage") },
        { json: "path", js: "path", typ: r("Destination") },
    ], false),
    "TypesSDPath": o([
        { json: "$schema", js: "$schema", typ: "" },
        { json: "description", js: "description", typ: "" },
        { json: "oneOf", js: "oneOf", typ: a(r("SDPathOneOf")) },
        { json: "title", js: "title", typ: "" },
    ], false),
    "SDPathOneOf": o([
        { json: "additionalProperties", js: "additionalProperties", typ: true },
        { json: "description", js: "description", typ: "" },
        { json: "properties", js: "properties", typ: r("OneOfProperties") },
        { json: "required", js: "required", typ: a("") },
        { json: "type", js: "type", typ: "" },
    ], false),
    "OneOfProperties": o([
        { json: "Physical", js: "Physical", typ: u(undefined, r("Physical")) },
        { json: "Content", js: "Content", typ: u(undefined, r("Content")) },
    ], false),
    "Content": o([
        { json: "properties", js: "properties", typ: r("ContentProperties") },
        { json: "required", js: "required", typ: a("") },
        { json: "type", js: "type", typ: "" },
    ], false),
    "ContentProperties": o([
        { json: "content_id", js: "content_id", typ: r("ID") },
    ], false),
    "ID": o([
        { json: "description", js: "description", typ: "" },
        { json: "format", js: "format", typ: "" },
        { json: "type", js: "type", typ: "" },
    ], false),
    "Physical": o([
        { json: "properties", js: "properties", typ: r("PhysicalProperties") },
        { json: "required", js: "required", typ: a("") },
        { json: "type", js: "type", typ: "" },
    ], false),
    "PhysicalProperties": o([
        { json: "device_id", js: "device_id", typ: r("ID") },
        { json: "path", js: "path", typ: r("Path") },
    ], false),
    "Path": o([
        { json: "description", js: "description", typ: "" },
        { json: "type", js: "type", typ: "" },
    ], false),
    "SDPathBatch": o([
        { json: "$schema", js: "$schema", typ: "" },
        { json: "definitions", js: "definitions", typ: r("SDPathBatchDefinitions") },
        { json: "description", js: "description", typ: "" },
        { json: "properties", js: "properties", typ: r("SDPathBatchProperties") },
        { json: "required", js: "required", typ: a("") },
        { json: "title", js: "title", typ: "" },
        { json: "type", js: "type", typ: "" },
    ], false),
    "SDPathBatchDefinitions": o([
        { json: "SdPath", js: "SdPath", typ: r("DefinitionsSDPath") },
    ], false),
    "DefinitionsSDPath": o([
        { json: "description", js: "description", typ: "" },
        { json: "oneOf", js: "oneOf", typ: a(r("SDPathOneOf")) },
    ], false),
    "SDPathBatchProperties": o([
        { json: "paths", js: "paths", typ: r("Paths") },
    ], false),
    "ActionType": [
        "core_action",
        "library_action",
    ],
    "QueryType": [
        "query",
    ],
};
