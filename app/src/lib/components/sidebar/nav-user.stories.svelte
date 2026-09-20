<script module lang="ts">
	import { defineMeta } from "@storybook/addon-svelte-csf";
	import NavUser from "./nav-user.svelte";
	import { SidebarProvider } from "$lib/components/ui/sidebar/index.js";

	const { Story } = defineMeta({
		title: "Sidebar/NavUser",
		component: NavUser,
		tags: ["autodocs"],
	});
</script>

<Story name="Default">
	<SidebarProvider>
		<div class="w-64 bg-sidebar p-2">
			<NavUser />
		</div>
	</SidebarProvider>
</Story>
