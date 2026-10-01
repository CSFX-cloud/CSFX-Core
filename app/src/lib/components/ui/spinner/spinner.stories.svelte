<script module lang="ts">
	import { defineMeta } from "@storybook/addon-svelte-csf";
	import { Spinner } from "./index.js";

	const { Story } = defineMeta({
		title: "ui/Spinner",
		component: Spinner,
		tags: ["autodocs"],
	});
</script>

<Story name="Default" />

<Story name="Large">
	<Spinner class="size-8" />
</Story>
