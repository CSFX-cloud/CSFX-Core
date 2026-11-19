<script lang="ts" generics="T extends string = string">
	import { Select } from "bits-ui";
	import { Check, ChevronDown } from "@lucide/svelte";

	type Option = { value: T; label: string };

	let {
		value = $bindable("" as T),
		options,
		id,
		placeholder = "Select",
		disabled = false,
		onchange
	}: {
		value?: T;
		options: Option[];
		id?: string;
		placeholder?: string;
		disabled?: boolean;
		onchange?: (value: T) => void;
	} = $props();

	const selected = $derived(options.find((option) => option.value === value));
</script>

<Select.Root
	type="single"
	{value}
	{disabled}
	items={options}
	onValueChange={(next) => {
		value = next as T;
		onchange?.(next as T);
	}}
>
	<Select.Trigger {id} class="cx-input cx-select__trigger">
		<span class="cx-select__value" data-placeholder={selected ? undefined : ""}>
			{selected?.label ?? placeholder}
		</span>
		<ChevronDown class="cx-select__chevron" size={16} />
	</Select.Trigger>
	<Select.Content class="cx-select__content" sideOffset={6} align="start">
		<Select.Viewport class="cx-select__viewport">
			{#each options as option (option.value)}
				<Select.Item value={option.value} label={option.label} class="cx-select__item">
					{#snippet children({ selected: isSelected })}
						<span class="cx-select__check">
							{#if isSelected}<Check size={14} />{/if}
						</span>
						<span>{option.label}</span>
					{/snippet}
				</Select.Item>
			{/each}
		</Select.Viewport>
	</Select.Content>
</Select.Root>

<style>
	:global(.cx-select__trigger) {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: 8px;
		text-align: left;
		cursor: pointer;
	}
	:global(.cx-select__trigger[data-disabled]) {
		opacity: 0.5;
		cursor: not-allowed;
	}
	:global(.cx-select__trigger[data-state="open"]) {
		border-color: var(--ink);
		box-shadow: 0 0 0 3px var(--focus-halo);
	}
	:global(.cx-select__value) {
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}
	:global(.cx-select__value[data-placeholder]) {
		color: var(--ink-muted);
	}
	:global(.cx-select__chevron) {
		flex-shrink: 0;
		color: var(--ink-muted);
		transition: transform 0.16s ease;
	}
	:global(.cx-select__trigger[data-state="open"] .cx-select__chevron) {
		transform: rotate(180deg);
	}
	:global(.cx-select__content) {
		z-index: 60;
		display: grid;
		gap: 2px;
		min-width: var(--bits-select-anchor-width);
		max-height: min(320px, var(--bits-select-content-available-height));
		overflow-y: auto;
		padding: 6px;
		background: var(--bg-200);
		border: 1px solid var(--border-200);
		border-radius: var(--radius-md);
		box-shadow: var(--shadow-window);
		transform-origin: var(--bits-select-content-transform-origin);
	}
	:global(.cx-select__content[data-state="open"]) {
		animation: cx-select-in 0.16s cubic-bezier(0.16, 1, 0.3, 1);
	}
	:global(.cx-select__content[data-state="closed"]) {
		animation: cx-select-out 0.12s ease-in forwards;
	}
	:global(.cx-select__item) {
		display: flex;
		align-items: center;
		gap: 10px;
		height: 34px;
		padding: 0 10px;
		border-radius: var(--radius-sm);
		font: 400 13px/1 var(--font-sans);
		color: var(--ink-soft);
		cursor: pointer;
		user-select: none;
	}
	:global(.cx-select__item[data-highlighted]) {
		background: var(--bg-300);
		color: var(--ink);
	}
	:global(.cx-select__item[data-selected]) {
		color: var(--ink);
	}
	:global(.cx-select__check) {
		display: inline-flex;
		width: 14px;
		flex-shrink: 0;
	}
	@keyframes cx-select-in {
		from {
			opacity: 0;
			transform: translateY(-4px) scale(0.96);
		}
	}
	@keyframes cx-select-out {
		to {
			opacity: 0;
			transform: translateY(-4px) scale(0.96);
		}
	}
	@media (prefers-reduced-motion: reduce) {
		:global(.cx-select__content) {
			animation: none !important;
		}
	}
</style>
