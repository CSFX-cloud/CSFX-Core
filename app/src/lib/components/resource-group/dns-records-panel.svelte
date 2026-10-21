<script lang="ts">
    import { untrack } from "svelte";
    import Trash2Icon from "@lucide/svelte/icons/trash-2";
    import { auth } from "$lib/auth/store.svelte";
    import { Button } from "$lib/components/ui/button/index.js";
    import { createDnsRecord, deleteDnsRecord, getDnsZone, listDnsRecords, type DnsRecord, type DnsZone } from "$lib/api/ingress";

    let { groupId }: { groupId: string } = $props();

    let records = $state<DnsRecord[]>([]);
    let zone = $state<DnsZone | null>(null);
    let busy = $state(false);
    let error = $state<string | null>(null);
    let loadStarted = false;

    let name = $state("");
    let recordType = $state<"A" | "CNAME">("A");
    let value = $state("");

    const canSubmit = $derived(name.trim() !== "" && value.trim() !== "" && !busy);

    $effect(() => {
        if (auth.token && !loadStarted) {
            loadStarted = true;
            untrack(refresh);
        }
    });

    async function refresh() {
        if (!auth.token) return;
        try {
            [records, zone] = await Promise.all([listDnsRecords(auth.token, groupId), getDnsZone(auth.token)]);
            error = null;
        } catch (e) {
            error = e instanceof Error ? e.message : "Failed to load dns records";
        }
    }

    async function handleCreate() {
        if (!auth.token) return;
        busy = true;
        try {
            await createDnsRecord(auth.token, groupId, { name: name.trim(), record_type: recordType, value: value.trim() });
            name = "";
            value = "";
            await refresh();
        } catch (e) {
            error = e instanceof Error ? e.message : "Failed to create dns record";
        } finally {
            busy = false;
        }
    }

    async function handleDelete(record: DnsRecord) {
        if (!auth.token) return;
        if (!confirm(`Remove ${record.name}?`)) return;
        try {
            await deleteDnsRecord(auth.token, groupId, record.id);
            await refresh();
        } catch (e) {
            error = e instanceof Error ? e.message : "Failed to delete dns record";
        }
    }
</script>

<section class="flex flex-col gap-3">
    <div>
        <h2 class="text-base font-semibold tracking-tight">Local DNS</h2>
        <p class="text-sm text-muted-foreground mt-0.5">
            {#if zone}
                The platform serves the zone <span class="font-mono">{zone.zone}</span>{zone.servers.length > 0 ? ` from ${zone.servers.join(", ")}` : ""}.
                Gateway endpoints and routes inside the zone resolve automatically, the control plane answers to <span class="font-mono">{zone.console_host}</span>.
            {:else}
                The platform serves a local zone. Gateway endpoints and routes inside it resolve automatically.
            {/if}
            Trust the cluster CA to use HTTPS: <a class="underline" href="/api/setup/ca.crt" download="csfx-ca.crt">download CA</a>.
        </p>
    </div>

    {#if error}
        <p class="text-xs text-destructive">{error}</p>
    {/if}

    <div class="grid gap-3 rounded-lg border p-4 sm:grid-cols-4 sm:items-end">
        <div class="flex flex-col gap-1">
            <label class="text-xs text-muted-foreground" for="dns-name">Name</label>
            <input id="dns-name" class="border rounded px-3 py-1.5 text-sm bg-background" placeholder={`db.${zone?.zone ?? "csfx.internal"}`} bind:value={name} />
        </div>
        <div class="flex flex-col gap-1">
            <label class="text-xs text-muted-foreground" for="dns-type">Type</label>
            <select id="dns-type" class="border rounded px-3 py-1.5 text-sm bg-background" bind:value={recordType}>
                <option value="A">A</option>
                <option value="CNAME">CNAME</option>
            </select>
        </div>
        <div class="flex flex-col gap-1">
            <label class="text-xs text-muted-foreground" for="dns-value">Value</label>
            <input id="dns-value" class="border rounded px-3 py-1.5 text-sm bg-background" placeholder={recordType === "A" ? "10.0.0.9" : "target.example.com"} bind:value={value} />
        </div>
        <div>
            <Button size="sm" onclick={handleCreate} disabled={!canSubmit}>Add record</Button>
        </div>
    </div>

    {#if records.length === 0}
        <p class="text-sm text-muted-foreground">No manual records yet.</p>
    {:else}
        <div class="border rounded-lg overflow-hidden">
            <table class="w-full text-sm">
                <thead class="bg-muted/30 text-xs text-muted-foreground">
                    <tr>
                        <th class="px-4 py-2 text-left font-medium">Name</th>
                        <th class="px-4 py-2 text-left font-medium">Type</th>
                        <th class="px-4 py-2 text-left font-medium">Value</th>
                        <th class="px-4 py-2 text-left font-medium">TTL</th>
                        <th class="px-4 py-2"></th>
                    </tr>
                </thead>
                <tbody>
                    {#each records as record}
                        <tr class="border-t">
                            <td class="px-4 py-2 font-mono text-xs">{record.name}</td>
                            <td class="px-4 py-2">{record.record_type}</td>
                            <td class="px-4 py-2 font-mono text-xs">{record.value}</td>
                            <td class="px-4 py-2">{record.ttl}</td>
                            <td class="px-4 py-2 text-right">
                                <Button variant="ghost" size="icon-sm" onclick={() => handleDelete(record)} aria-label="Remove record" title="Remove record">
                                    <Trash2Icon class="size-4" />
                                </Button>
                            </td>
                        </tr>
                    {/each}
                </tbody>
            </table>
        </div>
    {/if}
</section>
