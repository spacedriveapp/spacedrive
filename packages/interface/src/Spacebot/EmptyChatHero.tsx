export function EmptyChatHero() {
	return (
		<div className="mb-6 text-left">
			<h1 className="text-ink text-[2.65rem] font-semibold tracking-tight">
				Let&apos;s get to work, James
			</h1>
			<p className="text-ink-dull mt-2 text-sm">
				Learn how to be productive with Spacebot. {''}
				<a
					href="https://github.com/spacedriveapp/spacebot"
					target="_blank"
					rel="noreferrer"
					className="text-ink-dull hover:text-ink underline underline-offset-4 transition-colors"
				>
					Read the docs.
				</a>
			</p>
		</div>
	);
}
