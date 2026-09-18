<script module lang="ts">
	import { defineMeta } from "@storybook/addon-svelte-csf";
	import {
		Sheet,
		SheetTrigger,
		SheetContent,
		SheetHeader,
		SheetTitle,
		SheetDescription,
		SheetFooter,
	} from "./index.js";
	import { Button } from "$lib/components/ui/button/index.js";

	const { Story } = defineMeta({
		title: "ui/Sheet",
		component: Sheet,
		tags: ["autodocs"],
	});
</script>

<Story name="Default">
	<Sheet>
		<SheetTrigger>
			{#snippet child({ props })}
				<Button {...props}>Open node details</Button>
			{/snippet}
		</SheetTrigger>
		<SheetContent>
			<SheetHeader>
				<SheetTitle>csfx-node-01</SheetTitle>
				<SheetDescription>Node details and metrics</SheetDescription>
			</SheetHeader>
			<SheetFooter>
				<Button variant="outline">Close</Button>
			</SheetFooter>
		</SheetContent>
	</Sheet>
</Story>
