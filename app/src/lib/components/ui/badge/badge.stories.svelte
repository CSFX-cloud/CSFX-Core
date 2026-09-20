<script module lang="ts">
	import { defineMeta } from "@storybook/addon-svelte-csf";
	import { Badge, badgeVariants } from "./index.js";

	const { Story } = defineMeta({
		title: "ui/Badge",
		component: Badge,
		tags: ["autodocs"],
		argTypes: {
			variant: {
				control: "select",
				options: Object.keys(badgeVariants.variants.variant),
			},
			loading: { control: "boolean" },
		},
		args: {
			variant: "default",
			loading: false,
		},
	});
</script>

<Story name="Default">
	{#snippet template(args)}
		<Badge {...args}>Badge</Badge>
	{/snippet}
</Story>

<Story name="AllVariants">
	<div class="flex flex-wrap items-center gap-2">
		<Badge variant="default">Default</Badge>
		<Badge variant="secondary">Secondary</Badge>
		<Badge variant="outline">Outline</Badge>
		<Badge variant="success">Success</Badge>
		<Badge variant="warning">Warning</Badge>
		<Badge variant="destructive">Destructive</Badge>
		<Badge variant="pending">Pending</Badge>
	</div>
</Story>

<Story name="Loading" args={{ loading: true }}>
	{#snippet template(args)}
		<Badge {...args}>Loading</Badge>
	{/snippet}
</Story>
