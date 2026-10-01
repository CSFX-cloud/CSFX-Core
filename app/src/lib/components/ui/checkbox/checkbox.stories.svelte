<script module lang="ts">
	import { defineMeta } from "@storybook/addon-svelte-csf";
	import { Checkbox } from "./index.js";

	const { Story } = defineMeta({
		title: "ui/Checkbox",
		component: Checkbox,
		tags: ["autodocs"],
		argTypes: {
			checked: { control: "boolean" },
			indeterminate: { control: "boolean" },
			disabled: { control: "boolean" },
		},
		args: {
			checked: false,
			indeterminate: false,
			disabled: false,
		},
	});
</script>

<Story name="Default" />

<Story name="Checked" args={{ checked: true }} />

<Story name="Indeterminate" args={{ indeterminate: true }} />

<Story name="Disabled" args={{ disabled: true }} />
