import { useEffect } from 'react';
import { forms } from '@sd/ui';
import { Heading } from '../Layout';
import Setting from '../Setting';

const { Form, Switch, useZodForm, z } = forms;

const schema = z.object({
	uiAnimations: z.boolean(),
	syncThemeWithSystem: z.boolean(),
	blurEffects: z.boolean()
});

export default function AppearanceSettings() {
	const form = useZodForm({
		schema
	});

	const onSubmit = form.handleSubmit(async (data) => {
		console.log({ data });
	});

	useEffect(() => {
		const subscription = form.watch(() => onSubmit());
		return () => subscription.unsubscribe();
	}, [form, onSubmit]);

	return (
		<Form form={form} onSubmit={onSubmit}>
			<Heading title="Appearance" description="Change the look of your client." />
			<Setting
				mini
				title="Sync Theme with System"
				description="The theme of the client will change based on your system theme."
			>
				<Switch {...form.register('syncThemeWithSystem')} className="m-2 ml-4" />
			</Setting>

			<Setting
				mini
				title="UI Animations"
				description="Dialogs and other UI elements will animate when opening and closing."
			>
				<Switch {...form.register('uiAnimations')} className="m-2 ml-4" />
			</Setting>
			<Setting
				mini
				title="Blur Effects"
				description="Some components will have a blur effect applied to them."
			>
				<Switch {...form.register('blurEffects')} className="m-2 ml-4" />
			</Setting>
		</Form>
	);
}
