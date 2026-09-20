<script module lang="ts">
	import { defineMeta } from "@storybook/addon-svelte-csf";
	import { Separator } from "./index.js";

	const { Story } = defineMeta({
		title: "ui/Separator",
		component: Separator,
		tags: ["autodocs"],
	});
</script>

<Story name="Horizontal">
	<div class="w-64">
		<p class="text-sm">Above</p>
		<Separator class="my-2" />
		<p class="text-sm">Below</p>
	</div>
</Story>

<Story name="Vertical">
	<div class="flex h-8 items-center gap-2">
		<span class="text-sm">Left</span>
		<Separator orientation="vertical" />
		<span class="text-sm">Right</span>
	</div>
</Story>
