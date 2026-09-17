<script module lang="ts">
	import { defineMeta } from "@storybook/addon-svelte-csf";
	import { Collapsible, CollapsibleTrigger, CollapsibleContent } from "./index.js";
	import { Button } from "$lib/components/ui/button/index.js";

	const { Story } = defineMeta({
		title: "ui/Collapsible",
		component: Collapsible,
		tags: ["autodocs"],
	});
</script>

<Story name="Default">
	<Collapsible class="w-72">
		<CollapsibleTrigger>
			{#snippet child({ props })}
				<Button {...props} variant="outline" size="sm">Toggle details</Button>
			{/snippet}
		</CollapsibleTrigger>
		<CollapsibleContent class="mt-2 rounded-md border border-border p-3 text-sm">
			Additional node details rendered here.
		</CollapsibleContent>
	</Collapsible>
</Story>
