<script module lang="ts">
	import { defineMeta } from "@storybook/addon-svelte-csf";
	import VncConsole from "./vnc-console.svelte";

	const { Story } = defineMeta({
		title: "Components/VncConsole",
		component: VncConsole,
		tags: ["autodocs"],
	});
</script>

<Story name="Default" args={{ token: "storybook-preview-token", workloadId: "preview-workload" }}>
	{#snippet template(args)}
		<div class="h-96 w-full">
			<VncConsole {...args} />
		</div>
	{/snippet}
</Story>
