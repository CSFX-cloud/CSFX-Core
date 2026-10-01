<script module lang="ts">
	import { defineMeta } from "@storybook/addon-svelte-csf";
	import StatusBadge from "./status-badge.svelte";

	const { Story } = defineMeta({
		title: "Components/StatusBadge",
		component: StatusBadge,
		tags: ["autodocs"],
		argTypes: {
			status: { control: "text" },
			label: { control: "text" },
		},
		args: {
			status: "running",
		},
	});
</script>

<Story name="Default" />

<Story name="AllStatuses">
	<div class="flex flex-wrap items-center gap-2">
		<StatusBadge status="running" />
		<StatusBadge status="pending" />
		<StatusBadge status="scheduled" />
		<StatusBadge status="creating" />
		<StatusBadge status="starting" />
		<StatusBadge status="pulling" />
		<StatusBadge status="stopped" />
		<StatusBadge status="failed" />
		<StatusBadge status="error" />
		<StatusBadge status="deleting" />
	</div>
</Story>

<Story name="LabelOverride" args={{ status: "running", label: "Healthy" }} />
