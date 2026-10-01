<script module lang="ts">
	import { defineMeta } from "@storybook/addon-svelte-csf";
	import NavMain from "./nav-main.svelte";
	import { SidebarProvider, SidebarMenu } from "$lib/components/ui/sidebar/index.js";
	import Server from "@lucide/svelte/icons/server";
	import Layers from "@lucide/svelte/icons/layers";

	const { Story } = defineMeta({
		title: "Sidebar/NavMain",
		component: NavMain,
		tags: ["autodocs"],
	});

	const items = [
		{ title: "Nodes", url: "/nodes", icon: Server, isActive: true },
		{ title: "Resource groups", url: "/resource-groups", icon: Layers },
	];
</script>

<Story name="Default">
	<SidebarProvider>
		<div class="w-64 bg-sidebar p-2">
			<NavMain {items} />
		</div>
	</SidebarProvider>
</Story>
