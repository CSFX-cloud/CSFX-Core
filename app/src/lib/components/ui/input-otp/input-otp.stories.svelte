<script module lang="ts">
	import { defineMeta } from "@storybook/addon-svelte-csf";
	import { InputOTP, InputOTPGroup, InputOTPSlot, InputOTPSeparator } from "./index.js";

	const { Story } = defineMeta({
		title: "ui/InputOTP",
		component: InputOTP,
		tags: ["autodocs"],
	});
</script>

<Story name="Default">
	<InputOTP maxlength={6}>
		{#snippet children({ cells })}
			<InputOTPGroup>
				{#each cells.slice(0, 3) as cell (cell)}
					<InputOTPSlot {cell} />
				{/each}
			</InputOTPGroup>
			<InputOTPSeparator />
			<InputOTPGroup>
				{#each cells.slice(3, 6) as cell (cell)}
					<InputOTPSlot {cell} />
				{/each}
			</InputOTPGroup>
		{/snippet}
	</InputOTP>
</Story>
