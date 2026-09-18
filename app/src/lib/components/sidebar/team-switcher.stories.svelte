<script module lang="ts">
	import { defineMeta } from "@storybook/addon-svelte-csf";
	import TeamSwitcher from "./team-switcher.svelte";
	import { SidebarProvider } from "$lib/components/ui/sidebar/index.js";
	import ShipWheel from "@lucide/svelte/icons/ship-wheel";

	const { Story } = defineMeta({
		title: "Sidebar/TeamSwitcher",
		component: TeamSwitcher,
		tags: ["autodocs"],
	});

	const teams = [{ name: "CSFX", logo: ShipWheel, plan: "Self-hosted" }];
</script>

<Story name="Default">
	<SidebarProvider>
		<div class="w-64 bg-sidebar p-2">
			<TeamSwitcher {teams} />
		</div>
	</SidebarProvider>
</Story>
