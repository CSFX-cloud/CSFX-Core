<script module lang="ts">
	import { defineMeta } from "@storybook/addon-svelte-csf";
	import { Button, buttonVariants } from "./index.js";

	const { Story } = defineMeta({
		title: "ui/Button",
		component: Button,
		tags: ["autodocs"],
		argTypes: {
			variant: {
				control: "select",
				options: Object.keys(buttonVariants.variants.variant),
			},
			size: {
				control: "select",
				options: Object.keys(buttonVariants.variants.size),
			},
			disabled: { control: "boolean" },
		},
		args: {
			variant: "default",
			size: "default",
			disabled: false,
		},
	});
</script>

<Story name="Default" args={{ children: "Button" }}>
	{#snippet template(args)}
		<Button {...args}>{args.children}</Button>
	{/snippet}
</Story>

<Story name="AllVariants">
	<div class="flex flex-wrap items-center gap-2">
		<Button variant="default">Default</Button>
		<Button variant="secondary">Secondary</Button>
		<Button variant="outline">Outline</Button>
		<Button variant="ghost">Ghost</Button>
		<Button variant="destructive">Destructive</Button>
		<Button variant="link">Link</Button>
	</div>
</Story>

<Story name="AllSizes">
	<div class="flex flex-wrap items-center gap-2">
		<Button size="xs">Extra small</Button>
		<Button size="sm">Small</Button>
		<Button size="default">Default</Button>
		<Button size="lg">Large</Button>
	</div>
</Story>

<Story name="Disabled" args={{ children: "Disabled", disabled: true }}>
	{#snippet template(args)}
		<Button {...args}>{args.children}</Button>
	{/snippet}
</Story>
