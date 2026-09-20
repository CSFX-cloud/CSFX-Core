<script module lang="ts">
	import { defineMeta } from "@storybook/addon-svelte-csf";
	import AppSidebar from "./app-sidebar.svelte";
	import { SidebarProvider } from "$lib/components/ui/sidebar/index.js";

	const { Story } = defineMeta({
		title: "Sidebar/AppSidebar",
		component: AppSidebar,
		tags: ["autodocs"],
	});
</script>

<Story name="Default">
	<div class="h-[600px] overflow-hidden">
		<SidebarProvider>
			<AppSidebar />
		</SidebarProvider>
	</div>
</Story>
