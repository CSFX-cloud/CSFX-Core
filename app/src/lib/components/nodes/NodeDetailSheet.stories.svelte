<script module lang="ts">
	import { defineMeta } from "@storybook/addon-svelte-csf";
	import NodeDetailSheet from "./NodeDetailSheet.svelte";
	import type { Node } from "$lib/api/nodes";

	const { Story } = defineMeta({
		title: "Nodes/NodeDetailSheet",
		component: NodeDetailSheet,
		tags: ["autodocs"],
	});

	const node: Node = {
		id: "b7e6c1e0-1c2d-4b3a-9f8e-123456789abc",
		name: "csfx-node-01",
		hostname: "csfx-node-01.local",
		ip_address: "10.0.0.12",
		os_type: "linux",
		os_version: "NixOS 26.05",
		architecture: "x86_64",
		agent_version: "0.4.2",
		status: "running",
		last_heartbeat: new Date().toISOString(),
		registered_at: new Date().toISOString(),
		cordoned: false,
	};
</script>

<Story name="Open" args={{ node, open: true, onClose: () => {} }} />

<Story name="Closed" args={{ node: null, open: false, onClose: () => {} }} />
