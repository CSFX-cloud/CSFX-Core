<script lang="ts">
    import Icon from "@iconify/svelte";
    import { Copy } from "@lucide/svelte";
    import { toast } from "svelte-sonner";
    import SideDrawer from "$lib/components/shell/side-drawer.svelte";
    import Switch from "$lib/components/shell/switch.svelte";
    import type { IdentityProvider } from "$lib/api/identity-providers";
    import { presetFor } from "./identity-provider-presets";

    type Props = {
        provider: IdentityProvider | null;
        roleName: string;
        testing: boolean;
        onClose: () => void;
        onTest: () => void;
        onEdit: () => void;
        onDelete: (deleteUsers: boolean) => void;
    };

    let { provider, roleName, testing, onClose, onTest, onEdit, onDelete }: Props = $props();

    let confirmingDelete = $state(false);
    let deleteUsers = $state(false);

    const preset = $derived(provider ? presetFor(provider.slug, provider.display_name) : null);
    const rows = $derived(
        provider
            ? [
                  { label: "Issuer", value: provider.issuer_url, mono: true },
                  { label: "Client ID", value: provider.client_id, mono: true },
                  { label: "Scopes", value: provider.scopes, mono: true },
                  { label: "Claims", value: `${provider.username_claim}, ${provider.email_claim}, ${provider.groups_claim}`, mono: true },
                  { label: "Default role", value: roleName, mono: false },
                  { label: "Create users on first sign-in", value: provider.auto_provision ? "Yes" : "No", mono: false },
                  { label: "Users", value: String(provider.user_count), mono: false },
              ]
            : [],
    );

    $effect(() => {
        if (provider) {
            confirmingDelete = false;
            deleteUsers = false;
        }
    });

    async function copyRedirectUri() {
        if (!provider) return;
        await navigator.clipboard.writeText(provider.redirect_uri);
        toast.success("Redirect URI copied");
    }
</script>

<SideDrawer open={provider !== null} title={provider?.display_name ?? ""} subtitle={provider?.slug} onclose={onClose}>
    {#snippet header()}
        {#if preset}<span class="svc-tile" style="background: var(--bg-200)"><Icon icon={preset.icon} width={20} height={20} color={preset.iconColor} /></span>{/if}
    {/snippet}
    {#if provider}
        <dl class="nd-kv">
            <div>
                <dt>Status</dt>
                <dd><span class="cx-badge {provider.enabled ? 'cx-badge--success' : ''}">{provider.enabled ? "Enabled" : "Disabled"}</span></dd>
            </div>
            {#each rows as row (row.label)}
                <div><dt>{row.label}</dt><dd class:mono={row.mono} style="word-break: break-all">{row.value}</dd></div>
            {/each}
            <div>
                <dt>Redirect URI</dt>
                <dd>
                    <button class="cx-btn cx-btn--ghost cx-btn--sm cx-mono" type="button" style="height: auto; padding: 4px 8px; font-size: 12px; white-space: normal; word-break: break-all" title="Copy redirect URI" onclick={copyRedirectUri}>
                        {provider.redirect_uri}
                        <Copy class="cx-i" aria-hidden="true" />
                    </button>
                </dd>
            </div>
        </dl>

        <div class="db-actions">
            <button class="cx-btn cx-btn--secondary cx-btn--sm" type="button" disabled={testing} aria-busy={testing} onclick={onTest}>
                {testing ? "Testing" : "Test connection"}
            </button>
            <button class="cx-btn cx-btn--secondary cx-btn--sm" type="button" onclick={onEdit}>Edit</button>
        </div>

        {#if confirmingDelete}
            <div class="st-note st-split">
                <span>
                    {#if provider.user_count > 0}
                        Also delete {provider.user_count} {provider.user_count === 1 ? "user" : "users"} and their SSH keys
                    {:else}
                        No users belong to this provider
                    {/if}
                </span>
                <Switch checked={deleteUsers} label="Delete users too" disabled={provider.user_count === 0} onchange={(value) => (deleteUsers = value)} />
            </div>
            <div class="db-actions">
                <button class="cx-btn cx-btn--primary cx-btn--sm nd-danger" type="button" onclick={() => onDelete(deleteUsers)}>Confirm delete</button>
                <button class="cx-btn cx-btn--ghost cx-btn--sm" type="button" onclick={() => (confirmingDelete = false)}>Cancel</button>
            </div>
        {:else}
            <div>
                <button class="cx-btn cx-btn--secondary cx-btn--sm nd-danger" type="button" onclick={() => (confirmingDelete = true)}>Delete provider</button>
            </div>
        {/if}
    {/if}
</SideDrawer>
