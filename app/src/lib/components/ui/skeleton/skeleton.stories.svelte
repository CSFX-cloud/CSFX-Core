<script module lang="ts">
	import { defineMeta } from "@storybook/addon-svelte-csf";
	import { Skeleton } from "./index.js";

	const { Story } = defineMeta({
		title: "ui/Skeleton",
		component: Skeleton,
		tags: ["autodocs"],
	});
</script>

<Story name="Default">
	<Skeleton class="h-4 w-48" />
</Story>

<Story name="Card">
	<div class="flex flex-col gap-2">
		<Skeleton class="h-32 w-64 rounded-lg" />
		<Skeleton class="h-4 w-40" />
		<Skeleton class="h-4 w-24" />
	</div>
</Story>

<Story name="Avatar">
	<Skeleton class="size-10 rounded-full" />
</Story>
