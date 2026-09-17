<script module lang="ts">
	import { defineMeta } from "@storybook/addon-svelte-csf";
	import AccountSettings from "./account-settings.svelte";

	const { Story } = defineMeta({
		title: "Settings/AccountSettings",
		component: AccountSettings,
		tags: ["autodocs"],
	});
</script>

<Story name="Default">
	<div class="w-[480px]">
		<AccountSettings />
	</div>
</Story>
