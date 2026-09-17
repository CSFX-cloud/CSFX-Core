<script module lang="ts">
	import { defineMeta } from "@storybook/addon-svelte-csf";
	import RightPanel from "./right-panel.svelte";

	const { Story } = defineMeta({
		title: "Auth/RightPanel",
		component: RightPanel,
		tags: ["autodocs"],
	});
</script>

<Story name="Default">
	<div class="h-[700px] bg-background">
		<RightPanel />
	</div>
</Story>
