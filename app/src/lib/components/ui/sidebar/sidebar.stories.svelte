<script module lang="ts">
	import { defineMeta } from "@storybook/addon-svelte-csf";
	import {
		SidebarProvider,
		Sidebar,
		SidebarHeader,
		SidebarContent,
		SidebarFooter,
		SidebarGroup,
		SidebarGroupLabel,
		SidebarGroupContent,
		SidebarMenu,
		SidebarMenuItem,
		SidebarMenuButton,
		SidebarInset,
		SidebarTrigger,
	} from "./index.js";

	const { Story } = defineMeta({
		title: "ui/Sidebar",
		component: Sidebar,
		tags: ["autodocs"],
	});
</script>

<Story name="Default">
	<div class="h-[500px] w-full overflow-hidden rounded-lg border border-border">
		<SidebarProvider>
			<Sidebar collapsible="none">
				<SidebarHeader>
					<span class="px-2 text-sm font-semibold">CSFX</span>
				</SidebarHeader>
				<SidebarContent>
					<SidebarGroup>
						<SidebarGroupLabel>Platform</SidebarGroupLabel>
						<SidebarGroupContent>
							<SidebarMenu>
								<SidebarMenuItem>
									<SidebarMenuButton isActive>Nodes</SidebarMenuButton>
								</SidebarMenuItem>
								<SidebarMenuItem>
									<SidebarMenuButton>Buckets</SidebarMenuButton>
								</SidebarMenuItem>
								<SidebarMenuItem>
									<SidebarMenuButton>Resource groups</SidebarMenuButton>
								</SidebarMenuItem>
								<SidebarMenuItem>
									<SidebarMenuButton>Logs</SidebarMenuButton>
								</SidebarMenuItem>
							</SidebarMenu>
						</SidebarGroupContent>
					</SidebarGroup>
				</SidebarContent>
				<SidebarFooter>
					<span class="px-2 text-xs text-muted-foreground">cedric.stillecke</span>
				</SidebarFooter>
			</Sidebar>
			<SidebarInset>
				<div class="flex items-center gap-2 border-b border-border p-2">
					<SidebarTrigger />
					<span class="text-sm">Page content</span>
				</div>
			</SidebarInset>
		</SidebarProvider>
	</div>
</Story>
