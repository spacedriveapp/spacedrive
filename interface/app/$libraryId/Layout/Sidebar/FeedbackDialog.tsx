import clsx from 'clsx';
import { useState } from 'react';
import { Dialog, TextArea, UseDialogProps, useDialog } from '@sd/ui';
import { useZodForm, z } from '@sd/ui/src/forms';
import { showAlertDialog } from '~/components';

const schema = z.object({
	feedback: z.string().min(1),
	emoji: z.string().max(1)
});

type formType = z.infer<typeof schema>;

export default function FeedbackDialog(props: UseDialogProps) {
	const form = useZodForm({ schema: schema });
	const emojis = ['🤩', '😀', '🙁', '😭'];
	const [emojiSelected, setEmojiSelected] = useState<string | undefined>(undefined);
	const feedback_url = 'https://spacedrive.com/api/feedback';

	const emojiSelectHandler = (index: number) => {
		setEmojiSelected(emojis[index]);
		form.setValue('emoji', emojis[index] as string);
	};

	const formSubmit = async (data: formType) => {
		try {
			await fetch(feedback_url, {
				method: 'POST',
				body: JSON.stringify(data)
			});
		} catch (error) {
			showAlertDialog({
				title: 'Error',
				value: 'There was an error submitting your feedback. Please try again.'
			});
		}
	};
	const watchForm = form.watch();

	return (
		<Dialog
			title="Feedback"
			dialog={useDialog(props)}
			form={form}
			onSubmit={form.handleSubmit(formSubmit)}
			submitDisabled={form.formState.isSubmitting || !watchForm.feedback || !watchForm.emoji}
			ctaLabel="Submit"
			closeLabel="Cancel"
			buttonsSideContent={
				<div className="flex w-full items-center justify-center gap-1">
					{emojis.map((emoji, i) => (
						<div
							onClick={() => emojiSelectHandler(i)}
							key={i.toString()}
							className={clsx(
								emojiSelected === emoji ? 'bg-green-600' : 'bg-app-input',
								'flex h-7 w-7 cursor-pointer items-center justify-center rounded-full border border-app-line transition-all duration-200 hover:scale-125'
							)}
						>
							{emoji}
						</div>
					))}
				</div>
			}
		>
			<TextArea
				placeholder="Your feedback..."
				className="w-full"
				{...form.register('feedback')}
			/>
		</Dialog>
	);
}
