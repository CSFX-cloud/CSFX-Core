<script module lang="ts">
	import { defineMeta } from "@storybook/addon-svelte-csf";
	import {
		DropdownMenu,
		DropdownMenuTrigger,
		DropdownMenuContent,
		DropdownMenuItem,
		DropdownMenuLabel,
		DropdownMenuSeparator,
	} from "./index.js";
	import { Button } from "$lib/components/ui/button/index.js";

	const { Story } = defineMeta({
		title: "ui/DropdownMenu",
		component: DropdownMenu,
		tags: ["autodocs"],
	});
</script>

<Story name="Default">
	<DropdownMenu>
		<DropdownMenuTrigger>
			{#snippet child({ props })}
				<Button {...props} variant="outline">Actions</Button>
			{/snippet}
		</DropdownMenuTrigger>
		<DropdownMenuContent>
			<DropdownMenuLabel>Node</DropdownMenuLabel>
			<DropdownMenuSeparator />
			<DropdownMenuItem>Restart</DropdownMenuItem>
			<DropdownMenuItem>Drain</DropdownMenuItem>
			<DropdownMenuItem>Delete</DropdownMenuItem>
		</DropdownMenuContent>
	</DropdownMenu>
</Story>
