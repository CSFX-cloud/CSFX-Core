<script module lang="ts">
	import { defineMeta } from "@storybook/addon-svelte-csf";
	import type { ReleaseNotes } from "$lib/api/system";
	import ReleaseNotesDrawer from "./release-notes-drawer.svelte";

	const { Story } = defineMeta({
		title: "Shell/ReleaseNotesDrawer",
		component: ReleaseNotesDrawer,
		parameters: { layout: "fullscreen" },
	});

	const releaseUrl = "https://github.com/CSFX-cloud/CSFX-Core/releases/tag/v0.3.0";

	const fullNotes: ReleaseNotes = {
		version: "0.3.0",
		channel: "stable",
		date: "2026-11-08",
		breaking: [],
		sections: {
			new: [
				{ type: "heading", text: "Patch notes in the update panel" },
				{ type: "paragraph", text: "See what changed in a release before you update. Notes appear when you open the update hint in the sidebar." },
				{ type: "image", src: "data:image/svg+xml;utf8,%3Csvg xmlns='http://www.w3.org/2000/svg' width='720' height='360'%3E%3Crect width='720' height='360' fill='%23222'/%3E%3C/svg%3E", alt: "The patch notes panel in dark mode" },
				{ type: "heading", text: "Update checks every 30 minutes" },
				{ type: "paragraph", text: "The sidebar checks for new releases in the background and follows your release channel." },
			],
			improved: [{ type: "list", items: ["Faster cluster overview on large clusters", "Clearer error messages when a node is unreachable"] }],
			fixed: [{ type: "list", items: ["Update hint no longer shows while updates are paused", "Node list no longer flickers during refresh", "Vault grants are applied after a restart"] }],
			security: [{ type: "list", items: ["Session tokens are rotated after a password change"] }],
		},
	};

	const breakingNotes: ReleaseNotes = {
		...fullNotes,
		version: "0.3.0-alpha.2",
		channel: "pre",
		breaking: ["Etcd keys under /csfx/config moved. Run csfx-migrate before upgrading.", "The legacy /v1/nodes endpoint was removed."],
	};

	const noop = () => {};
</script>

<Story name="Stable">
	<ReleaseNotesDrawer open version="0.3.0" prerelease={false} htmlUrl={releaseUrl} notes={fullNotes} onclose={noop} />
</Story>

<Story name="Pre-release with breaking changes">
	<ReleaseNotesDrawer open version="0.3.0-alpha.2" prerelease htmlUrl={releaseUrl} notes={breakingNotes} onclose={noop} />
</Story>

<Story name="Loading">
	<ReleaseNotesDrawer open version="0.3.0" prerelease={false} htmlUrl={releaseUrl} notes={null} loading onclose={noop} />
</Story>

<Story name="Fallback">
	<ReleaseNotesDrawer open version="0.2.9" prerelease={false} htmlUrl={releaseUrl} notes={null} onclose={noop} />
</Story>
