<script lang="ts">
    import { untrack } from "svelte";
    import { Lock, LockOpen, ShieldCheck, Trash2 } from "@lucide/svelte";
    import { goto } from "$app/navigation";
    import { page } from "$app/stores";
    import { auth } from "$lib/auth/store.svelte";
    import {
        deleteVault,
        getVault,
        listGrants,
        listSecrets,
        listVaultAudit,
        setVaultSealed,
        type SecretSummary,
        type VaultAuditEntry,
        type VaultGrant,
        type VaultOverview,
    } from "$lib/api/vaults";
    import { useResourceGroupState } from "$lib/components/resource-group/rg-state.svelte";
    import { breadcrumb } from "$lib/components/shell/breadcrumb.svelte";
    import ServiceIcon from "$lib/components/shell/service-icon.svelte";
    import VaultAccessTab from "$lib/components/vault/vault-access-tab.svelte";
    import VaultSecretsTab from "$lib/components/vault/vault-secrets-tab.svelte";
    import WorkloadTabs from "$lib/components/workload/workload-tabs.svelte";

    type Tab = "secrets" | "access";

    const TABS: { id: Tab; label: string }[] = [
        { id: "secrets", label: "Secrets" },
        { id: "access", label: "Access" },
    ];

    const rg = useResourceGroupState();
    const vaultId = $page.params.vaultId ?? "";
    const base = `/resource-groups/${rg.rgId}`;

    let vault = $state<VaultOverview | null>(null);
    let secrets = $state<SecretSummary[]>([]);
    let grants = $state<VaultGrant[]>([]);
    let audit = $state<VaultAuditEntry[]>([]);
    let tab = $state<Tab>("secrets");
    let loading = $state(true);
    let busy = $state(false);
    let error = $state<string | null>(null);

    async function load() {
        if (!auth.token) return;
        try {
            vault = await getVault(auth.token, rg.rgId, vaultId);
            [secrets, grants, audit] = await Promise.all([
                vault.sealed ? Promise.resolve([]) : listSecrets(auth.token, rg.rgId, vaultId),
                listGrants(auth.token, rg.rgId, vaultId),
                listVaultAudit(auth.token, rg.rgId, vaultId),
            ]);
            error = null;
        } catch (e) {
            error = e instanceof Error ? e.message : "Failed to load vault";
        } finally {
            loading = false;
        }
    }

    async function toggleSeal() {
        if (!auth.token || !vault) return;
        const sealing = !vault.sealed;
        if (sealing && !confirm(`Seal ${vault.name}? Nobody can read or change secrets until it is unsealed.`)) return;
        busy = true;
        try {
            await setVaultSealed(auth.token, rg.rgId, vaultId, sealing);
            await load();
        } catch (e) {
            error = e instanceof Error ? e.message : "Failed to change vault state";
        } finally {
            busy = false;
        }
    }

    async function remove() {
        if (!auth.token || !vault || !confirm(`Delete ${vault.name} and all of its secrets? This cannot be undone.`)) return;
        busy = true;
        try {
            await deleteVault(auth.token, rg.rgId, vaultId);
            await rg.load();
            await goto(base);
        } catch (e) {
            error = e instanceof Error ? e.message : "Failed to delete vault";
            busy = false;
        }
    }

    $effect(() => {
        if (auth.token) untrack(load);
    });

    $effect(() => {
        breadcrumb.set(vault?.name ?? vaultId.slice(0, 8));
        return () => breadcrumb.set(null);
    });
</script>

<div class="db-content">
    {#if loading}
        <div class="db-skeleton sv-skeleton" aria-hidden="true"></div>
    {:else if !vault}
        <p class="cx-hint cx-hint--error">{error ?? "Vault not found"}</p>
    {:else}
        <div class="db-head">
            <div class="rs-name">
                <ServiceIcon name="key-vault" size="xl" />
                <div>
                    <h1 class="db-title">{vault.name}</h1>
                    <p class="db-sub">
                        <span class="cx-badge {vault.sealed ? 'cx-badge--warning' : 'cx-badge--success'}">{vault.sealed ? "Sealed" : "Unsealed"}</span>
                        <a class="cx-link" href={base}>{rg.group?.name ?? "Resource group"}</a>
                    </p>
                </div>
            </div>
            <div class="db-actions">
                <button class="cx-btn cx-btn--secondary cx-btn--sm" type="button" disabled={busy} onclick={toggleSeal}>
                    {#if vault.sealed}<LockOpen class="cx-i" aria-hidden="true" />Unseal vault{:else}<Lock class="cx-i" aria-hidden="true" />Seal vault{/if}
                </button>
                <button class="cx-icon-btn nd-danger" type="button" disabled={busy} aria-label="Delete vault" title="Delete vault" onclick={remove}>
                    <Trash2 class="cx-i" aria-hidden="true" />
                </button>
            </div>
        </div>

        <section class="db-stats" aria-label="Vault summary">
            <div class="cx-stat">
                <p class="cx-stat__label">Secrets</p>
                <p class="db-stat-value">{vault.secret_count}</p>
            </div>
            <div class="cx-stat">
                <p class="cx-stat__label">Reads &middot; 24 h</p>
                <p class="db-stat-value">{vault.reads_24h}</p>
                <div class="cx-stat__foot">{vault.denied_24h} denied</div>
            </div>
            <div class="cx-stat">
                <p class="cx-stat__label">Access grants</p>
                <p class="db-stat-value">{vault.grant_count}</p>
            </div>
            <div class="cx-stat">
                <p class="cx-stat__label">Soft delete</p>
                <p class="db-stat-value">{vault.soft_delete_days} days</p>
            </div>
        </section>

        <div class="kv-sec">
            <ShieldCheck class="cx-i" aria-hidden="true" />
            <span><b>Secrets are encrypted at rest.</b> Resources get them as environment variables or files at start. Nothing is stored in code or images.</span>
        </div>

        {#if error}<p class="cx-hint cx-hint--error">{error}</p>{/if}

        <WorkloadTabs tabs={TABS} active={tab} label="Vault sections" onselect={(id) => (tab = id)} />

        {#if tab === "secrets"}
            {#if vault.sealed}
                <p class="cx-hint">This vault is sealed. Unseal it to read or change secrets.</p>
            {:else}
                <VaultSecretsTab rgId={rg.rgId} {vaultId} {secrets} onChanged={load} onError={(message) => (error = message)} />
            {/if}
        {:else}
            <VaultAccessTab rgId={rg.rgId} {vaultId} {grants} {audit} softDeleteDays={vault.soft_delete_days} onChanged={load} onError={(message) => (error = message)} />
        {/if}
    {/if}
</div>
