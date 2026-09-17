<script module lang="ts">
	import { defineMeta } from "@storybook/addon-svelte-csf";
	import {
		Dialog,
		DialogTrigger,
		DialogContent,
		DialogTitle,
		DialogDescription,
	} from "./index.js";
	import { Button } from "$lib/components/ui/button/index.js";

	const { Story } = defineMeta({
		title: "ui/Dialog",
		component: Dialog,
		tags: ["autodocs"],
	});
</script>

<Story name="Default">
	<Dialog>
		<DialogTrigger>
			{#snippet child({ props })}
				<Button {...props}>Delete node</Button>
			{/snippet}
		</DialogTrigger>
		<DialogContent>
			<DialogTitle>Delete node</DialogTitle>
			<DialogDescription>
				This permanently removes the node from the cluster. This action cannot be undone.
			</DialogDescription>
		</DialogContent>
	</Dialog>
</Story>
