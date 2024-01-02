/*
 * This file exports a object which contains Different Kinds of Icons.
 */
import { type FC as FunctionComponent, type LazyExoticComponent, type SVGProps } from 'react';

import * as Code from './Code';
import * as Extras from './Extras';

export const LayeredIcons: Partial<
	Record<string, Record<string, LazyExoticComponent<FunctionComponent<SVGProps<SVGSVGElement>>>>>
> = { Code, Extras };
