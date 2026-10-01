<script module lang="ts">
	import { defineMeta } from "@storybook/addon-svelte-csf";
	import UpdateSettings from "./update-settings.svelte";

	const { Story } = defineMeta({
		title: "Settings/UpdateSettings",
		component: UpdateSettings,
		tags: ["autodocs"],
	});
</script>

<Story name="Default">
	<div class="w-[640px]">
		<UpdateSettings />
	</div>
</Story>
