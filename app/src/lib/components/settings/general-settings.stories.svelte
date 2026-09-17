<script module lang="ts">
	import { defineMeta } from "@storybook/addon-svelte-csf";
	import GeneralSettings from "./general-settings.svelte";

	const { Story } = defineMeta({
		title: "Settings/GeneralSettings",
		component: GeneralSettings,
		tags: ["autodocs"],
	});
</script>

<Story name="Default">
	<div class="w-[480px]">
		<GeneralSettings />
	</div>
</Story>
