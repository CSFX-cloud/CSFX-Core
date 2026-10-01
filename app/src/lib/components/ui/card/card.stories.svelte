<script module lang="ts">
	import { defineMeta } from "@storybook/addon-svelte-csf";
	import {
		Card,
		CardHeader,
		CardTitle,
		CardDescription,
		CardContent,
		CardFooter,
		CardAction,
	} from "./index.js";
	import { Button } from "$lib/components/ui/button/index.js";

	const { Story } = defineMeta({
		title: "ui/Card",
		component: Card,
		tags: ["autodocs"],
	});
</script>

<Story name="Default">
	<Card class="w-80">
		<CardHeader>
			<CardTitle>Node metrics</CardTitle>
			<CardDescription>Last 24 hours</CardDescription>
			<CardAction>
				<Button variant="ghost" size="icon-sm">...</Button>
			</CardAction>
		</CardHeader>
		<CardContent>
			<p class="text-sm text-muted-foreground">CPU 42%, Memory 61%</p>
		</CardContent>
		<CardFooter>
			<Button variant="outline" size="sm">View details</Button>
		</CardFooter>
	</Card>
</Story>
