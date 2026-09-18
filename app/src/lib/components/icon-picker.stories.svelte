<script module lang="ts">
	import { defineMeta } from "@storybook/addon-svelte-csf";
	import IconPicker from "./icon-picker.svelte";

	const { Story } = defineMeta({
		title: "Components/IconPicker",
		component: IconPicker,
		tags: ["autodocs"],
		argTypes: {
			icon: { control: "text" },
			color: { control: "color" },
		},
		args: {
			icon: "mdi:server",
			color: "#6366f1",
		},
	});
</script>

<Story name="Default">
	{#snippet template(args)}
		<div class="w-96">
			<IconPicker {...args} />
		</div>
	{/snippet}
</Story>
