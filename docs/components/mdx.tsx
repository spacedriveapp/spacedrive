import {Accordion, Accordions} from 'fumadocs-ui/components/accordion';
import {Step, Steps} from 'fumadocs-ui/components/steps';
import {Tab, Tabs} from 'fumadocs-ui/components/tabs';
import defaultMdxComponents from 'fumadocs-ui/mdx';
import type {MDXComponents} from 'mdx/types';
import {FlowDiagram} from './flow-diagram';

export function getMDXComponents(components?: MDXComponents) {
	return {
		...defaultMdxComponents,
		Steps,
		Step,
		Tabs,
		Tab,
		Accordions,
		Accordion,
		FlowDiagram,
		...components
	} satisfies MDXComponents;
}

export const useMDXComponents = getMDXComponents;

declare global {
	type MDXProvidedComponents = ReturnType<typeof getMDXComponents>;
}
