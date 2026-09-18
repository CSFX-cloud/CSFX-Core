<script module lang="ts">
	import { defineMeta } from "@storybook/addon-svelte-csf";
	import { Input } from "./index.js";

	const { Story } = defineMeta({
		title: "ui/Input",
		component: Input,
		tags: ["autodocs"],
		argTypes: {
			type: { control: "text" },
			placeholder: { control: "text" },
			disabled: { control: "boolean" },
		},
		args: {
			type: "text",
			placeholder: "Enter text...",
			disabled: false,
		},
	});
</script>

<Story name="Default" />

<Story name="Disabled" args={{ disabled: true, placeholder: "Disabled" }} />

<Story name="WithValue" args={{ value: "csfx-node-01" }} />

<Story name="File" args={{ type: "file" }} />
