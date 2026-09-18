<script module lang="ts">
	import { defineMeta } from "@storybook/addon-svelte-csf";
	import { Switch } from "./index.js";

	const { Story } = defineMeta({
		title: "ui/Switch",
		component: Switch,
		tags: ["autodocs"],
		argTypes: {
			checked: { control: "boolean" },
			disabled: { control: "boolean" },
		},
		args: {
			checked: false,
			disabled: false,
		},
	});
</script>

<Story name="Default" />

<Story name="Checked" args={{ checked: true }} />

<Story name="Disabled" args={{ disabled: true }} />
