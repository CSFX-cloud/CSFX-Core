<script module lang="ts">
	import { defineMeta } from "@storybook/addon-svelte-csf";
	import NavProjects from "./nav-projects.svelte";
	import { SidebarProvider } from "$lib/components/ui/sidebar/index.js";
	import Container from "@lucide/svelte/icons/container";
	import ShipWheel from "@lucide/svelte/icons/ship-wheel";

	const { Story } = defineMeta({
		title: "Sidebar/NavProjects",
		component: NavProjects,
		tags: ["autodocs"],
	});

	const projects = [
		{ name: "Docker", url: "#", icon: Container },
		{ name: "Kubernetes", url: "#", icon: ShipWheel },
	];
</script>

<Story name="Default">
	<SidebarProvider>
		<div class="w-64 bg-sidebar p-2">
			<NavProjects {projects} />
		</div>
	</SidebarProvider>
</Story>
