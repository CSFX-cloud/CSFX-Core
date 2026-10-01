<script module lang="ts">
	import { defineMeta } from "@storybook/addon-svelte-csf";
	import { Tooltip, TooltipTrigger, TooltipContent, TooltipProvider } from "./index.js";
	import { Button } from "$lib/components/ui/button/index.js";

	const { Story } = defineMeta({
		title: "ui/Tooltip",
		component: Tooltip,
		tags: ["autodocs"],
	});
</script>

<Story name="Default">
	<TooltipProvider>
		<Tooltip>
			<TooltipTrigger>
				{#snippet child({ props })}
					<Button {...props} variant="outline" size="sm">Hover me</Button>
				{/snippet}
			</TooltipTrigger>
			<TooltipContent>Node is running</TooltipContent>
		</Tooltip>
	</TooltipProvider>
</Story>
