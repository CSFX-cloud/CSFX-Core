<script lang="ts">
    import type { Snippet } from "svelte";
    import { ChevronLeft } from "@lucide/svelte";
    import { useResourceGroupState } from "$lib/components/resource-group/rg-state.svelte";
    import { RESOURCE_TYPES, type ResourceTypeKey } from "$lib/components/resource-group/resource-types";
    import { breadcrumb } from "$lib/components/shell/breadcrumb.svelte";
    import ServiceIcon from "$lib/components/shell/service-icon.svelte";

    let { type, title, children }: { type: ResourceTypeKey; title: string; children: Snippet } = $props();

    const rg = useResourceGroupState();
    const resource = $derived(RESOURCE_TYPES.find((candidate) => candidate.key === type));

    $effect(() => {
        breadcrumb.set(rg.group ? `${rg.group.name} / Add resource` : null);
        return () => breadcrumb.set(null);
    });
</script>

<div class="db-content">
    <section class="cx-stage" aria-label={title}>
        <div class="cx-stage__canvas"></div>
        <div class="cp-head">
            <div style="display: grid; gap: 14px">
                <a class="ga-back" href="/resource-groups/{rg.rgId}/add">
                    <ChevronLeft class="cx-i" aria-hidden="true" />
                    Back to add resource
                </a>
                <div class="cp-title">
                    {#if resource}<ServiceIcon name={resource.service} size="xl" />{/if}
                    <div>
                        <h1>{title}</h1>
                        <p>into {rg.group?.name ?? "resource group"}{rg.group ? ` · ${rg.group.internal_cidr}` : ""}</p>
                    </div>
                </div>
            </div>
        </div>
    </section>

    <section class="cx-panel" aria-label={title}>
        {@render children()}
    </section>
</div>
