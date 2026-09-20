<script module lang="ts">
	import { defineMeta } from "@storybook/addon-svelte-csf";
	import { Label } from "./index.js";
	import { Input } from "$lib/components/ui/input/index.js";

	const { Story } = defineMeta({
		title: "ui/Label",
		component: Label,
		tags: ["autodocs"],
	});
</script>

<Story name="Default">
	<Label>Node name</Label>
</Story>

<Story name="WithInput">
	<div class="grid gap-1.5">
		<Label for="node-name">Node name</Label>
		<Input id="node-name" placeholder="csfx-node-01" />
	</div>
</Story>
