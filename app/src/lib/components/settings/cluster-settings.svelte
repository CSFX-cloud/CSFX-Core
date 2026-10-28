<script lang="ts">
    import { onMount } from "svelte";
    import SettingsPage from "$lib/components/settings/settings-page.svelte";
    import { auth } from "$lib/auth/store.svelte";
    import { getClusterVip, updateClusterVip } from "$lib/api/cluster";

    const DEFAULT_PREFIX_LENGTH = 24;
    const DEFAULT_ROUTER_ID = 51;
    const IPV4_PATTERN = /^(\d{1,3})\.(\d{1,3})\.(\d{1,3})\.(\d{1,3})$/;

    let address = $state("");
    let prefixLength = $state(String(DEFAULT_PREFIX_LENGTH));
    let routerId = $state(String(DEFAULT_ROUTER_ID));
    let stored = $state("");
    let loading = $state(true);
    let saving = $state(false);
    let error = $state<string | null>(null);
    let saved = $state(false);

    const snapshot = $derived(`${address.trim()}|${prefixLength}|${routerId}`);

    function validate(): string | null {
        const octets = IPV4_PATTERN.exec(address.trim())?.slice(1).map(Number);
        if (!octets || octets.some((octet) => octet > 255)) return "address must be an ipv4 address";
        const prefix = Number(prefixLength);
        if (!Number.isInteger(prefix) || prefix < 8 || prefix > 30) return "prefix length must be between 8 and 30";
        const router = Number(routerId);
        if (!Number.isInteger(router) || router < 1 || router > 255) return "router id must be between 1 and 255";
        return null;
    }

    async function load() {
        if (!auth.token) return;
        loading = true;
        try {
            const vip = await getClusterVip(auth.token);
            if (vip) {
                address = vip.address;
                prefixLength = String(vip.prefix_length);
                routerId = String(vip.router_id);
            }
            stored = snapshot;
            error = null;
        } catch (e) {
            error = e instanceof Error ? e.message : "failed to load floating address";
        } finally {
            loading = false;
        }
    }

    async function save() {
        if (!auth.token) return;
        const problem = validate();
        if (problem) {
            error = problem;
            return;
        }
        saving = true;
        saved = false;
        try {
            await updateClusterVip(auth.token, {
                address: address.trim(),
                prefix_length: Number(prefixLength),
                router_id: Number(routerId),
            });
            stored = snapshot;
            error = null;
            saved = true;
        } catch (e) {
            error = e instanceof Error ? e.message : "failed to save floating address";
        } finally {
            saving = false;
        }
    }

    onMount(load);
</script>

{#if error}<p class="cx-hint cx-hint--error">{error}</p>{/if}

<SettingsPage title="Cluster" description="High availability for the control plane.">
    <section class="cx-panel st-panel" aria-labelledby="st-vip">
        <div>
            <h3 id="st-vip">Floating address</h3>
            <p>
                One address that always points to a healthy master. Pick an unused address in the same subnet as all masters. Masters apply a change within about a minute and restart their gateway
                briefly. Object storage replication is fixed when the cluster is created and is not changed by this setting. Failover needs three masters.
            </p>
        </div>
        {#if loading}
            <div class="db-skeleton db-skeleton--row" style="width: 200px" aria-hidden="true"></div>
        {:else}
            <form
                class="cx-field"
                style="max-width: 420px"
                onsubmit={(event) => {
                    event.preventDefault();
                    save();
                }}
            >
                <label class="cx-label" for="vip-address">Address</label>
                <input class="cx-input" id="vip-address" type="text" placeholder="192.168.1.50" bind:value={address} />
                <label class="cx-label" for="vip-prefix">Prefix length</label>
                <input class="cx-input" id="vip-prefix" type="number" min="8" max="30" bind:value={prefixLength} />
                <label class="cx-label" for="vip-router">Router id</label>
                <input class="cx-input" id="vip-router" type="number" min="1" max="255" bind:value={routerId} />
                <div class="st-row">
                    <button class="cx-btn cx-btn--secondary cx-btn--sm" type="submit" style="height: 44px" disabled={saving || !address.trim() || snapshot === stored} aria-busy={saving}>
                        {saving ? "Saving" : "Save"}
                    </button>
                </div>
                {#if saved && !saving}<p class="cx-hint" style="color: var(--success)">Saved</p>{/if}
            </form>
        {/if}
    </section>
</SettingsPage>
