import { useCallback, useMemo } from 'react';
import { NavigateOptions, useSearchParams } from 'react-router-dom';
import { z } from 'zod';

export function useZodSearchParams<Z extends z.AnyZodObject>(schema: Z) {
	// eslint-disable-next-line no-restricted-syntax
	const [searchParams, setSearchParams] = useSearchParams();
	const typedSearchParams = useMemo(
		() => getParams(searchParams, schema),
		[searchParams, schema]
	);

	if (!typedSearchParams.success) throw typedSearchParams.errors;

	return [
		typedSearchParams.data as z.infer<Z>,
		useCallback(
			(
				data: z.input<Z> | ((data: z.input<Z>) => z.input<Z>),
				navigateOpts?: NavigateOptions
			) => {
				if (typeof data === 'function') {
					setSearchParams((params) => {
						const typedPrevParams = getParams(params, z.any());

						if (!typedPrevParams.success) throw typedPrevParams.errors;

						return schema.parse(data(typedPrevParams.data));
					}, navigateOpts);
				} else {
					setSearchParams(data as any, navigateOpts);
				}
			},
			[setSearchParams, schema]
		)
	] as const;
}

// from https://github.com/kiliman/remix-params-helper/blob/main/src/helper.ts
// original skips empty strings but empty strings are useful sometimes

export function getParams<T extends z.ZodType<any, any, any>>(
	params: URLSearchParams | FormData | Record<string, string | undefined>,
	schema: T
) {
	type ParamsType = z.infer<T>;
	return getParamsInternal<ParamsType>(params, schema);
}

function isIterable(maybeIterable: unknown): maybeIterable is Iterable<unknown> {
	return Symbol.iterator in Object(maybeIterable);
}

function getParamsInternal<T>(
	params: URLSearchParams | FormData | Record<string, string | undefined>,
	schema: any
):
	| { success: true; data: T; errors: undefined }
	| { success: false; data: undefined; errors: { [key: string]: string } } {
	const o: any = {};
	let entries: [string, unknown][] = [];
	if (isIterable(params)) {
		entries = Array.from(params);
	} else {
		entries = Object.entries(params);
	}
	for (const [key, value] of entries) {
		parseParams(o, schema, key, value);
	}

	const result = schema.safeParse(o);
	if (result.success) {
		return { success: true, data: result.data as T, errors: undefined };
	} else {
		const errors: Record<string, any> = {};
		const addError = (key: string, message: string) => {
			if (!Object.prototype.hasOwnProperty.call(errors, key)) {
				errors[key] = message;
			} else {
				if (!Array.isArray(errors[key])) {
					errors[key] = [errors[key]];
				}
				errors[key].push(message);
			}
		};
		for (const issue of result.error.issues) {
			const { message, path, code, expected, received } = issue;
			const [key, index] = path;
			let value = o[key];
			let prop = key;
			if (index !== undefined) {
				value = value[index];
				prop = `${key}[${index}]`;
			}
			addError(key, message);
		}
		return { success: false, data: undefined, errors };
	}
}

function parseParams(o: any, schema: any, key: string, value: any) {
	// find actual shape definition for this key
	let shape = schema;
	while (shape instanceof z.ZodObject || shape instanceof z.ZodEffects) {
		shape =
			shape instanceof z.ZodObject
				? shape.shape
				: shape instanceof z.ZodEffects
					? shape._def.schema
					: null;
		if (shape === null) {
			throw new Error(`Could not find shape for key ${key}`);
		}
	}

	if (key.includes('.')) {
		const [parentProp, ...rest] = key.split('.') as [string, ...string[]];
		o[parentProp!] = o[parentProp] ?? {};
		parseParams(o[parentProp], shape[parentProp], rest.join('.'), value);
		return;
	}
	let isArray = false;
	if (key.includes('[]')) {
		isArray = true;
		key = key.replace('[]', '');
	}
	const def = shape[key];
	if (def) {
		processDef(def, o, key, value as string);
	}
}

function processDef(def: z.ZodTypeAny, o: any, key: string, value: string) {
	let parsedValue: any;
	if (def instanceof z.ZodString || def instanceof z.ZodLiteral) {
		parsedValue = value;
	} else if (def instanceof z.ZodNumber) {
		const num = Number(value);
		parsedValue = isNaN(num) ? value : num;
	} else if (def instanceof z.ZodDate) {
		const date = Date.parse(value);
		parsedValue = isNaN(date) ? value : new Date(date);
	} else if (def instanceof z.ZodBoolean) {
		parsedValue = value === 'true' ? true : value === 'false' ? false : Boolean(value);
	} else if (def instanceof z.ZodNativeEnum || def instanceof z.ZodEnum) {
		parsedValue = value;
	} else if (def instanceof z.ZodOptional || def instanceof z.ZodDefault) {
		// def._def.innerType is the same as ZodOptional's .unwrap(), which unfortunately doesn't exist on ZodDefault
		processDef(def._def.innerType, o, key, value);
		// return here to prevent overwriting the result of the recursive call
		return;
	} else if (def instanceof z.ZodArray) {
		if (o[key] === undefined) {
			o[key] = [];
		}
		processDef(def.element, o, key, value);
		// return here since recursive call will add to array
		return;
	} else if (def instanceof z.ZodEffects) {
		processDef(def._def.schema, o, key, value);
		return;
	} else {
		throw new Error(`Unexpected type ${def._def.typeName} for key ${key}`);
	}
	if (Array.isArray(o[key])) {
		o[key].push(parsedValue);
	} else {
		o[key] = parsedValue;
	}
}
