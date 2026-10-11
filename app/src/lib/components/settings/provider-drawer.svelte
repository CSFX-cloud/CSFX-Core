<script lang="ts">
    import Icon from "@iconify/svelte";
    import CopyIcon from "@lucide/svelte/icons/copy";
    import { toast } from "svelte-sonner";
    import * as Sheet from "$lib/components/ui/sheet/index.js";
    import { Button } from "$lib/components/ui/button/index.js";
    import { Switch } from "$lib/components/ui/switch/index.js";
    import StatusBadge from "$lib/components/status-badge.svelte";
    import type { IdentityProvider } from "$lib/api/identity-providers";
    import { presetFor } from "./identity-provider-presets";

    let {
        provider,
        roleName,
        testing,
        onClose,
        onTest,
        onEdit,
        onDelete,
    }: {
        provider: IdentityProvider | null;
        roleName: string;
        testing: boolean;
        onClose: () => void;
        onTest: () => void;
        onEdit: () => void;
        onDelete: (deleteUsers: boolean) => void;
    } = $props();

    let confirmingDelete = $state(false);
    let deleteUsers = $state(false);

    const preset = $derived(provider ? presetFor(provider.slug, provider.display_name) : null);

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

<Sheet.Root open={provider !== null} onOpenChange={(open) => !open && onClose()}>
    <Sheet.Content class="sm:max-w-md">
        {#if provider && preset}
            <Sheet.Header class="pt-6">
                <div class="flex items-center gap-3">
                    <Icon icon={preset.icon} width={28} height={28} color={preset.iconColor} />
                    <div class="min-w-0">
                        <Sheet.Title class="truncate">{provider.display_name}</Sheet.Title>
                        <Sheet.Description class="truncate font-mono">{provider.slug}</Sheet.Description>
                    </div>
                </div>
            </Sheet.Header>

            <div class="flex flex-col gap-3 border-y border-dashed border-border px-4 py-5 text-xs">
                <div class="flex items-center justify-between gap-2">
                    <span class="text-muted-foreground">Status</span>
                    <StatusBadge
                        status={provider.enabled ? "active" : "disabled"}
                        label={provider.enabled ? "Enabled" : "Disabled"}
                    />
                </div>
                <div class="flex items-center justify-between gap-4">
                    <span class="text-muted-foreground">Issuer</span>
                    <span class="truncate font-mono" title={provider.issuer_url}>{provider.issuer_url}</span>
                </div>
                <div class="flex items-center justify-between gap-4">
                    <span class="text-muted-foreground">Client ID</span>
                    <span class="truncate font-mono">{provider.client_id}</span>
                </div>
                <div class="flex items-center justify-between gap-4">
                    <span class="text-muted-foreground">Redirect URI</span>
                    <button
                        class="flex min-w-0 items-center gap-1.5 rounded px-1.5 py-0.5 font-mono hover:bg-muted transition-colors"
                        onclick={copyRedirectUri}
                        title="Copy redirect URI"
                    >
                        <span class="truncate">{provider.redirect_uri}</span>
                        <CopyIcon class="size-3 shrink-0" />
                    </button>
                </div>
                <div class="flex items-center justify-between gap-4">
                    <span class="text-muted-foreground">Scopes</span>
                    <span class="truncate font-mono">{provider.scopes}</span>
                </div>
                <div class="flex items-center justify-between gap-4">
                    <span class="text-muted-foreground">Claims</span>
                    <span class="truncate font-mono">
                        {provider.username_claim}, {provider.email_claim}, {provider.groups_claim}
                    </span>
                </div>
                <div class="flex items-center justify-between gap-2">
                    <span class="text-muted-foreground">Default role</span>
                    <span>{roleName}</span>
                </div>
                <div class="flex items-center justify-between gap-2">
                    <span class="text-muted-foreground">Create users on first sign-in</span>
                    <span>{provider.auto_provision ? "Yes" : "No"}</span>
                </div>
                <div class="flex items-center justify-between gap-2">
                    <span class="text-muted-foreground">Users</span>
                    <span>{provider.user_count}</span>
                </div>
            </div>

            <div class="flex gap-2 px-4">
                <Button variant="outline" onclick={onTest} disabled={testing}>
                    {testing ? "Testing..." : "Test connection"}
                </Button>
                <Button variant="outline" onclick={onEdit}>Edit</Button>
            </div>

            <Sheet.Footer class="mt-auto">
                {#if confirmingDelete}
                    <div class="flex items-center justify-between gap-3 rounded-md border border-destructive/30 bg-destructive/5 px-3 py-2">
                        <span class="text-sm">
                            {#if provider.user_count > 0}
                                Also delete {provider.user_count} {provider.user_count === 1 ? "user" : "users"} and their SSH keys
                            {:else}
                                No users belong to this provider
                            {/if}
                        </span>
                        <Switch bind:checked={deleteUsers} disabled={provider.user_count === 0} />
                    </div>
                    <Button variant="destructive" onclick={() => onDelete(deleteUsers)}>Confirm delete</Button>
                    <Button variant="ghost" onclick={() => (confirmingDelete = false)}>Cancel</Button>
                {:else}
                    <Button
                        variant="outline"
                        class="border-destructive/40 text-destructive hover:bg-destructive/10"
                        onclick={() => (confirmingDelete = true)}
                    >
                        Delete provider
                    </Button>
                {/if}
            </Sheet.Footer>
        {/if}
    </Sheet.Content>
</Sheet.Root>
