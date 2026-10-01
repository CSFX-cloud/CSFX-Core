<script module lang="ts">
	import { defineMeta } from "@storybook/addon-svelte-csf";
	import UpdateCard from "./update-card.svelte";
	import { SidebarProvider } from "$lib/components/ui/sidebar/index.js";

	const { Story } = defineMeta({
		title: "Sidebar/UpdateCard",
		component: UpdateCard,
		tags: ["autodocs"],
	});
</script>

<Story name="Default">
	<SidebarProvider>
		<div class="w-64 bg-sidebar p-2">
			<UpdateCard />
		</div>
	</SidebarProvider>
</Story>
