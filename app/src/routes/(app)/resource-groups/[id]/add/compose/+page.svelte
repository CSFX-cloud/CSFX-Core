<script lang="ts">
    import { ChevronLeft } from "@lucide/svelte";
    import { goto } from "$app/navigation";
    import ComposeEditor from "$lib/components/resource-group/compose-editor.svelte";
    import { useResourceGroupState } from "$lib/components/resource-group/rg-state.svelte";
    import { breadcrumb } from "$lib/components/shell/breadcrumb.svelte";
    import ServiceIcon from "$lib/components/shell/service-icon.svelte";

    const rg = useResourceGroupState();
    const addHref = `/resource-groups/${rg.rgId}/add`;

    async function done() {
        await rg.load();
        goto(`/resource-groups/${rg.rgId}`);
    }

    $effect(() => {
        breadcrumb.set(rg.group ? `${rg.group.name} / Add resource` : null);
        return () => breadcrumb.set(null);
    });
</script>

<div class="db-content">
    <section class="cx-stage" aria-label="Deploy a Compose stack">
        <div class="cx-stage__canvas"></div>
        <div class="cp-head">
            <div style="display: grid; gap: 14px">
                <a class="ga-back" href={addHref}>
                    <ChevronLeft class="cx-i" aria-hidden="true" />
                    Back to add resource
                </a>
                <div class="cp-title">
                    <ServiceIcon name="docker-compose" size="xl" />
                    <div>
                        <h1>Deploy a Compose stack</h1>
                        <p>into {rg.group?.name ?? "resource group"}{rg.group ? ` · ${rg.group.internal_cidr}` : ""}</p>
                    </div>
                </div>
            </div>
        </div>
    </section>

    <ComposeEditor rgId={rg.rgId} groupName={rg.group?.name ?? "the group"} onDone={done} oncancel={() => goto(addHref)} />
</div>
