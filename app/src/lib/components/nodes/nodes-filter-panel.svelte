<script lang="ts">
    import * as Collapsible from "$lib/components/ui/collapsible/index.js";
    import { Checkbox } from "$lib/components/ui/checkbox/index.js";
    import { Label } from "$lib/components/ui/label/index.js";
    import ChevronDownIcon from "@lucide/svelte/icons/chevron-down";
    import type { Node } from "$lib/api/nodes";

    interface FacetOption {
        value: string;
        count: number;
    }

    let {
        nodes,
        statusFilter = $bindable(new Set<string>()),
        osFilter = $bindable(new Set<string>()),
        archFilter = $bindable(new Set<string>()),
    }: {
        nodes: Node[];
        statusFilter?: Set<string>;
        osFilter?: Set<string>;
        archFilter?: Set<string>;
    } = $props();

    function countBy(values: string[]): FacetOption[] {
        const counts = new Map<string, number>();
        for (const value of values) {
            counts.set(value, (counts.get(value) ?? 0) + 1);
        }
        return [...counts.entries()]
            .map(([value, count]) => ({ value, count }))
            .sort((a, b) => b.count - a.count);
    }

    const statusOptions = $derived(countBy(nodes.map((n) => n.status)));
    const osOptions = $derived(countBy(nodes.map((n) => n.os_type)));
    const archOptions = $derived(countBy(nodes.map((n) => n.architecture)));

    function toggle(set: Set<string>, value: string): Set<string> {
        const next = new Set(set);
        if (next.has(value)) {
            next.delete(value);
        } else {
            next.add(value);
        }
        return next;
    }

    const hasActiveFilters = $derived(
        statusFilter.size > 0 || osFilter.size > 0 || archFilter.size > 0
    );

    function clearFilters() {
        statusFilter = new Set();
        osFilter = new Set();
        archFilter = new Set();
    }
</script>

{#snippet facetGroup(title: string, options: FacetOption[], selected: Set<string>, onToggle: (value: string) => void)}
    <Collapsible.Root open class="group border-b border-border px-4 py-3">
        <Collapsible.Trigger class="flex w-full items-center justify-between text-label uppercase text-muted-foreground tracking-[0.02em]">
            {title}
            <ChevronDownIcon class="size-3.5 transition-transform group-data-[state=closed]:-rotate-90" />
        </Collapsible.Trigger>
        <Collapsible.Content class="mt-2.5 flex flex-col gap-2">
            {#each options as option (option.value)}
                <Label class="flex items-center gap-2 text-sm font-normal cursor-pointer">
                    <Checkbox
                        checked={selected.has(option.value)}
                        onCheckedChange={() => onToggle(option.value)}
                    />
                    <span class="flex-1 truncate">{option.value}</span>
                    <span class="text-xs text-muted-foreground">{option.count}</span>
                </Label>
            {/each}
        </Collapsible.Content>
    </Collapsible.Root>
{/snippet}

<div class="flex h-full flex-col overflow-y-auto">
    <div class="flex items-center justify-between px-4 py-3 border-b border-border">
        <span class="text-label uppercase text-muted-foreground tracking-[0.02em]">Filters</span>
        {#if hasActiveFilters}
            <button
                class="text-xs text-primary hover:underline"
                onclick={clearFilters}
            >
                Clear
            </button>
        {/if}
    </div>

    {@render facetGroup("Status", statusOptions, statusFilter, (v) => (statusFilter = toggle(statusFilter, v)))}
    {@render facetGroup("Operating system", osOptions, osFilter, (v) => (osFilter = toggle(osFilter, v)))}
    {@render facetGroup("Architecture", archOptions, archFilter, (v) => (archFilter = toggle(archFilter, v)))}
</div>
