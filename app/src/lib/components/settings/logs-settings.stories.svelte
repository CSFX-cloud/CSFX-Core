<script module lang="ts">
	import { defineMeta } from "@storybook/addon-svelte-csf";
	import LogsSettings from "./logs-settings.svelte";

	const { Story } = defineMeta({
		title: "Settings/LogsSettings",
		component: LogsSettings,
		tags: ["autodocs"],
	});
</script>

<Story name="Default">
	<div class="w-[560px]">
		<LogsSettings />
	</div>
</Story>
