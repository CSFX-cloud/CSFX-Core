<script module lang="ts">
	import { defineMeta } from "@storybook/addon-svelte-csf";
	import { Toaster } from "./index.js";
	import { Button } from "$lib/components/ui/button/index.js";
	import { toast } from "svelte-sonner";

	const { Story } = defineMeta({
		title: "ui/Sonner",
		component: Toaster,
		tags: ["autodocs"],
	});
</script>

<Story name="Default">
	<Toaster />
	<Button onclick={() => toast("Node csfx-node-01 restarted")}>Show toast</Button>
</Story>
