<script module lang="ts">
	import { defineMeta } from "@storybook/addon-svelte-csf";
	import SettingsDialog from "./settings-dialog.svelte";
	import { Button } from "$lib/components/ui/button/index.js";
	import { settingsDialog } from "./settings-store.svelte.js";

	const { Story } = defineMeta({
		title: "Settings/SettingsDialog",
		component: SettingsDialog,
		tags: ["autodocs"],
	});
</script>

<Story name="Default">
	<Button onclick={() => settingsDialog.show()}>Open settings</Button>
	<SettingsDialog />
</Story>
