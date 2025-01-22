export const items: {
	when?: string;
	subtext?: string;
	completed?: boolean;
	title: string;
	description: string;
}[] = [
	{
		when: 'Big Bang',
		subtext: 'Q1 2022',
		completed: true,
		title: 'File discovery',
		description:
			'Scan devices, drives and cloud accounts to build a directory of all files with metadata.'
	},
	{
		title: 'Preview generation',
		completed: true,
		description: 'Auto generate lower resolution stand-ins for image and video.'
	},
	{
		title: 'Statistics',
		completed: true,
		description: 'Total capacity, index size, preview media size, free space etc.'
	},
	{
		title: 'Jobs',
		completed: true,
		description:
			'Tasks to be performed via a queue system with multi-threaded workers, such as indexing, identifying, generating preview media and moving files. With a Job Manager interface for tracking progress, pausing and restarting jobs.'
	},
	{
		completed: true,
		title: 'Explorer',
		description:
			'Browse online/offline storage locations, view files with metadata, perform basic CRUD.'
	},
	{
		completed: true,
		title: 'Self hosting',
		description:
			'Spacedrive can be deployed as a service via Docker, behaving as just another device powering your personal cloud.'
	},
	{
		completed: true,
		title: 'Tags',
		description:
			'Define routines on custom tags to automate workflows, easily tag files individually, in bulk and automatically via rules.'
	},
	{
		completed: true,
		title: 'Search',
		description:
			'Instantly search your library, including offline locations. Use advanced filters to refine and save searches for later.'
	},
	{
		completed: true,
		title: 'Quick view',
		description:
			'View images, videos and documents in a full screen modal with nested inspector and context switcher.'
	},
	{
		completed: true,
		title: 'Media view',
		description: 'Turn any directory into a camera roll including media from subdirectories'
	},
	{
		completed: true,
		title: 'Spacedrop',
		description: 'Drop files between devices and contacts on a keybind like AirDrop.'
	},
	{
		when: '0.2 Alpha',
		subtext: 'February 2024',
		title: 'Drag & drop',
		completed: true,
		description: 'Drag and drop files between devices and drives.'
	},
	{
		title: 'Language support',
		completed: true,
		description: 'Support for 12+ languages, with a community-driven translation via i18n.'
	},
	{
		title: 'Command Palette',
		completed: true,
		description: 'Quickly navigate to any file or folder from anywhere in the app.'
	},
	{
		title: 'Resizable Sidebar',
		completed: true,
		description:
			'Customize the sidebar width to your liking, with a toggle to hide it completely.'
	},
	{
		when: '0.3 Alpha',
		subtext: 'May 2024',
		title: 'Media File Metadata',
		description: 'View metadata for media files, for all common formats.',
		completed: true
	},
	{
		title: 'Move to Trash',
		completed: true,
		description:
			'Have the option to move files and folders to the trash, instead of deleting them permanently.'
	},
	{
		title: 'Tag Assign Mode',
		completed: true,
		description: 'Assign tags to multiple files and folders at once with hotkeys.'
	},
	{
		title: 'Estimated time remaining for Jobs',
		completed: true,
		description: 'See how long a job will take to complete.'
	},
	{
		when: '0.4 Alpha',
		subtext: 'June 2024',
		title: 'Local Server Protection',
		completed: true,
		description:
			"Protect local instances of Spacedrive's server from other clients on your network."
	},
	{
		title: 'Multi-device FS Actions',
		completed: true,
		description:
			'Support for creating, copying, moving, renaming, and deleting files and folders between devices.'
	},
	{
		title: 'OS Drag & Drop',
		completed: true,
		description: 'Drag and drop files and folders to and from the operating system.'
	},
	{
		title: 'New Overview Design',
		description:
			'New Overview design with a focus on the most important information about your library.',
		completed: true,
	},
	{
		when: '0.5 Beta',
		subtext: 'To be determined',
		title: 'Video thumbstrips',
		completed: false,
		description:
			'Generate and display thumbstrips for videos, with a scrubber to preview the video.'
	},
	{
		title: 'Spacedrive Cloud',
		completed: false,
		description:
			'Sync your library to the cloud, to be accessed from anywhere (mobile & desktop apps).'
	},
	{
		title: 'Peer-to-Peer Sync',
		completed: false,
		description:
			'Sync & Fetch files from your library with other devices on your local network.'
	},
	{
		title: 'iOS & Android release',
		completed: false,
		description:
			'Spacedrive will be available on the App Store and Google Play Store (TestFlight to start).'
	},
	{
		title: '3rd-Party Authentication',
		completed: false,
		description: 'Authenticate with Spacedrive using your Google, Apple, or Github accounts.'
	},
	{
		title: 'AI Object Detection',
		completed: false,
		description: 'Identify objects in images and videos assigning system labels.'
	},
	{
		title: 'OCR Text Extraction',
		completed: false,
		description: 'Extract text from images and PDFs, and documents.'
	},
	{
		title: 'AI Search',
		completed: false,
		description:
			'Search the contents of your files, including images, audio and video with a deep understanding of context and content.'
	},
	{
		title: 'Comments',
		completed: false,
		description:
			'Add comments to files and folders, with support for XY coordinates for photos and timestamp for videos.'
	},
	{
		when: '0.6 Beta',
		subtext: 'To be determined',
		title: 'Third-party cloud integrations',
		completed: false,
		description:
			'Filesystem integrations with iCloud, Google Drive, Dropbox, OneDrive & Mega + easy API for the community to add more.'
	},
	{
		title: 'Organizer',
		completed: false,
		description: 'Natural language file management.'
	},
	{
		title: 'Column view',
		completed: false,
		description: 'View files in the beloved column layout with a nested inspector, with trees!'
	},
	{
		title: 'Extensions',
		completed: false,
		description:
			'Build tools on top of Spacedrive, extend functionality and integrate third party services.'
	},
	{
		title: 'File versioning',
		completed: false,
		description:
			'Automatically save versions of files when they change, with a timeline view and the ability to restore.'
	},
	{
		title: 'CLI',
		completed: false,
		description:
			'Access Spacedrive from the command line, with a rich set of commands to manage your library and devices.'
	},
	{
		title: 'Advanced media analysis',
		completed: false,
		description: 'Transcribe audio, identify faces, video scenes and more.'
	},

	{
		title: 'File converter',
		completed: false,
		description: 'Convert image and video between common formats from the context menu.'
	},
	{
		when: '1.0 Release',
		subtext: 'To be determined',
		title: 'Security audit',
		completed: false,
		description:
			'We will hire a third party security firm to audit the codebase and ensure the highest level of security.'
	},
	{
		when: 'The Future',
		subtext: 'To be determined',
		title: 'Web portal',
		completed: false,
		description:
			'Access the web interface via the browser, remotely access your library and manage your devices and Spaces.'
	},
	{
		title: 'Spaces',
		completed: false,
		description:
			'Create and manage Spaces, hosted locally or on the cloud, to share with friends or publish on the web. Spaces are AI native, with a custom local language model that can converse with the user and puppeteer the Explorer view.'
	},
	{
		title: 'Key manager',
		completed: false,
		description:
			'View, mount, unmount and hide keys. Mounted keys can be used to instantly encrypt and decrypt any files on your node.'
	}
];
